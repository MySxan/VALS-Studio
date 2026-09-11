use crate::CancellationToken;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use vocal_domain::audio::{AudioSource, AudioUri, ContentHash};

#[derive(Debug)]
pub enum AudioError {
    Io(std::io::Error),
    Invalid(&'static str),
    Probe(symphonia::core::errors::Error),
    Cancelled,
    SourceChanged,
}
impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {self:?}", self.message_key())
    }
}
impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Probe(e) => Some(e),
            _ => None,
        }
    }
}
impl From<std::io::Error> for AudioError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl AudioError {
    pub fn message_key(&self) -> &'static str {
        match self {
            Self::Io(_) => "audio.io_failed",
            Self::Cancelled => "audio.cancelled",
            Self::SourceChanged => "audio.source_changed",
            _ => "audio.invalid_source",
        }
    }
}

impl From<vocal_analysis_api::AnalysisError> for AudioError {
    fn from(error: vocal_analysis_api::AnalysisError) -> Self {
        match error {
            vocal_analysis_api::AnalysisError::Cancelled => Self::Cancelled,
            _ => Self::Invalid("analysis contract error"),
        }
    }
}

/// Bounded-memory copy/hash. Cancellation also checked after the final read.
pub(crate) fn hash_copy(
    mut input: impl Read,
    mut output: impl Write,
    cancel: &CancellationToken,
) -> Result<(ContentHash, u64), AudioError> {
    let mut hasher = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        cancel.check()?;
        let count = input.read(&mut buffer)?;
        cancel.check()?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        total = total
            .checked_add(count as u64)
            .ok_or(AudioError::Invalid("source too large"))?;
    }
    Ok((ContentHash(hasher.finalize().into()), total))
}

/// Verifies the absolute fallback at call time. Relative relocation is not yet implemented.
/// A caller must still guard against changes between this check and subsequent use.
pub fn verify_source(source: &AudioSource, cancel: &CancellationToken) -> Result<(), AudioError> {
    cancel.check()?;
    let AudioUri::Linked {
        absolute_fallback, ..
    } = source.uri();
    let path = Path::new(absolute_fallback);
    if !path.is_absolute() {
        return Err(AudioError::Invalid(
            "source location requires relinking on this host",
        ));
    }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(AudioError::Invalid("source is not a regular file"));
    }
    let (hash, size) = hash_copy(file, std::io::sink(), cancel)?;
    if hash != source.content_hash() || size != source.size_bytes() {
        return Err(AudioError::SourceChanged);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct CancelDuringRead(CancellationToken);
    impl Read for CancelDuringRead {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            bytes[0] = 1;
            self.0.cancel();
            Ok(1)
        }
    }
    #[test]
    fn cancellation_during_io_never_publishes_a_partial_hash() {
        let cancel = CancellationToken::default();
        let mut output = Vec::new();
        assert!(matches!(
            hash_copy(CancelDuringRead(cancel.clone()), &mut output, &cancel),
            Err(AudioError::Cancelled)
        ));
        assert!(output.is_empty());
    }
    #[test]
    fn sha256_known_vector_and_multichunk_copy() {
        let (hash, size) =
            hash_copy(&b"abc"[..], std::io::sink(), &CancellationToken::default()).unwrap();
        assert_eq!(size, 3);
        assert_eq!(
            hash.to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let data = vec![7_u8; 150000];
        let mut copy = Vec::new();
        let (_, count) = hash_copy(&data[..], &mut copy, &CancellationToken::default()).unwrap();
        assert_eq!(count, data.len() as u64);
        assert_eq!(copy, data);
    }
}
