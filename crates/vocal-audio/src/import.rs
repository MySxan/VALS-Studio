use crate::{io::hash_copy, verify_source, AudioError, CancellationToken};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
use symphonia::core::{
    io::{MediaSource, MediaSourceStream},
    probe::Hint,
};
use vocal_domain::{
    audio::{AudioMetadata, AudioSource, AudioUri},
    identity::EntityId,
    time::Samples,
};

pub struct WavImporter;
impl WavImporter {
    /// Registers header metadata from an immutable temporary snapshot, not decoded PCM.
    /// Transient disk use equals input size; no full audio buffer is held in memory.
    pub fn import_linked(
        path: impl AsRef<Path>,
        cancel: &CancellationToken,
    ) -> Result<AudioSource, AudioError> {
        cancel.check()?;
        let path = path.as_ref().canonicalize()?;
        let absolute = path
            .to_str()
            .ok_or(AudioError::Invalid("source path must be UTF-8"))?
            .to_owned();
        let mut file = File::open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(AudioError::Invalid("source is not a regular file"));
        }
        let mut header = [0_u8; 12];
        file.read_exact(&mut header)?;
        if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
            return Err(AudioError::Invalid("expected RIFF WAVE"));
        }
        file.rewind()?;
        let mut snapshot = tempfile::tempfile()?;
        let (hash, size) = hash_copy(file, &mut snapshot, cancel)?;
        snapshot.rewind()?;
        snapshot.read_exact(&mut header)?;
        if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
            return Err(AudioError::SourceChanged);
        }
        snapshot.rewind()?;
        // RIFF's declared extent must fit the captured bytes; trailing chunks are preserved in hash.
        if u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64 + 8 > size {
            return Err(AudioError::Invalid("truncated RIFF container"));
        }
        let stream = MediaSourceStream::new(
            Box::new(CancellableFile {
                file: snapshot,
                cancel: cancel.clone(),
            }),
            Default::default(),
        );
        let mut hint = Hint::new();
        hint.with_extension("wav");
        let probed = symphonia::default::get_probe().format(
            &hint,
            stream,
            &Default::default(),
            &Default::default(),
        );
        cancel.check()?;
        let probed = probed.map_err(AudioError::Probe)?;
        let params = &probed
            .format
            .default_track()
            .ok_or(AudioError::Invalid("no WAV audio track"))?
            .codec_params;
        let rate = params
            .sample_rate
            .ok_or(AudioError::Invalid("missing sample rate"))?;
        let channels = params
            .channels
            .ok_or(AudioError::Invalid("missing channels"))?
            .count();
        if !(1..=2).contains(&channels) {
            return Err(AudioError::Invalid(
                "this importer supports mono/stereo WAV only",
            ));
        }
        let frames = params
            .n_frames
            .and_then(|n| i64::try_from(n).ok())
            .ok_or(AudioError::Invalid("missing or oversized frame count"))?;
        let metadata = AudioMetadata::new(rate, channels as u16, Samples::new(frames))
            .map_err(|e| AudioError::Invalid(e.0))?;
        let source = AudioSource::new(
            EntityId::new(),
            AudioUri::Linked {
                absolute_fallback: absolute,
                relative_path: None,
            },
            hash,
            size,
            metadata,
        )
        .map_err(|e| AudioError::Invalid(e.0))?;
        // Detect replacement/change during capture/probe before publishing the linked reference.
        verify_source(&source, cancel)?;
        Ok(source)
    }
}

pub(crate) struct CancellableFile {
    pub(crate) file: File,
    pub(crate) cancel: CancellationToken,
}
impl Read for CancellableFile {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(std::io::Error::other("cancelled"));
        }
        self.file.read(bytes)
    }
}
impl Seek for CancellableFile {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(position)
    }
}
impl MediaSource for CancellableFile {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        self.file.metadata().ok().map(|m| m.len())
    }
}
