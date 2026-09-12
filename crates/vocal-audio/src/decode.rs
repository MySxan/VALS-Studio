use crate::{import::CancellableFile, io::hash_copy, verify_source, AudioError};
use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
};
use symphonia::core::{
    audio::SampleBuffer, errors::Error as DecodeError, io::MediaSourceStream, probe::Hint,
};
use vocal_analysis_api::*;
use vocal_domain::{
    analysis::{Confidence, ConfidenceKind},
    audio::AudioUri,
    signal::Pcm,
};

pub struct DecodeAnalyzer;
fn map_error(error: impl std::fmt::Display) -> AnalysisError {
    AnalysisError::Provider(error.to_string())
}
fn audio_error(error: AudioError) -> AnalysisError {
    match error {
        AudioError::Cancelled => AnalysisError::Cancelled,
        AudioError::SourceChanged => AnalysisError::SourceChanged,
        other => map_error(other),
    }
}
/// Decode one independently verified source for a non-analysis consumer.
///
/// This deliberately bypasses the analysis cache: playback owns its buffer and
/// still receives the same source-integrity, format and memory-limit checks as
/// the decode analyzer.
pub fn decode_verified_pcm(
    source: &vocal_domain::audio::AudioSource,
    cancel: &CancellationToken,
) -> Result<Pcm, AnalysisError> {
    let analyzer = DecodeAnalyzer;
    let context = AnalysisContext::new(source.clone());
    analyzer.validate_inputs(&context, cancel)?;
    decode_pcm(&context, cancel, &ProgressSink::default())
}

fn decode_pcm(
    ctx: &AnalysisContext,
    cancel: &CancellationToken,
    progress: &ProgressSink,
) -> Result<Pcm, AnalysisError> {
    cancel.check()?;
    let metadata = ctx.source.metadata();
    let expected = (metadata.frames().get() as u64)
        .checked_mul(metadata.channels() as u64)
        .filter(|n| *n <= MAX_PCM_BYTES / 4)
        .ok_or(AnalysisError::LimitExceeded)? as usize;
    let AudioUri::Linked {
        absolute_fallback, ..
    } = ctx.source.uri();
    if !Path::new(absolute_fallback).is_absolute() {
        return Err(AnalysisError::Invalid("source requires relinking"));
    }
    let file = File::open(absolute_fallback).map_err(map_error)?;
    let mut snapshot = tempfile::tempfile().map_err(map_error)?;
    let (hash, size) = hash_copy(file, &mut snapshot, cancel).map_err(audio_error)?;
    if hash != ctx.source.content_hash() || size != ctx.source.size_bytes() {
        return Err(AnalysisError::SourceChanged);
    }
    snapshot.rewind().map_err(map_error)?;
    let mut header = [0_u8; 12];
    snapshot.read_exact(&mut header).map_err(map_error)?;
    if &header[..4] != b"RIFF"
        || &header[8..] != b"WAVE"
        || u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64 + 8 > size
    {
        return Err(AnalysisError::Invalid("invalid/truncated RIFF WAV"));
    }
    snapshot.rewind().map_err(map_error)?;
    let stream = MediaSourceStream::new(
        Box::new(CancellableFile {
            file: snapshot,
            cancel: cancel.clone(),
        }),
        Default::default(),
    );
    let mut hint = Hint::new();
    hint.with_extension("wav");
    let result = symphonia::default::get_probe().format(
        &hint,
        stream,
        &Default::default(),
        &Default::default(),
    );
    cancel.check()?;
    let mut format = result.map_err(map_error)?.format;
    let track = format
        .default_track()
        .ok_or(AnalysisError::Invalid("no audio track"))?;
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &Default::default())
        .map_err(map_error)?;
    let mut samples = Vec::with_capacity(expected);
    progress.report(0, metadata.frames().get() as u64);
    loop {
        cancel.check()?;
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(DecodeError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => {
                cancel.check()?;
                return Err(map_error(e));
            }
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = decoder.decode(&packet).map_err(map_error)?;
        if decoded.spec().rate != metadata.sample_rate()
            || decoded.spec().channels.count() != metadata.channels() as usize
        {
            return Err(AnalysisError::Invalid("decoded format differs from source"));
        }
        let count = decoded
            .frames()
            .checked_mul(metadata.channels() as usize)
            .ok_or(AnalysisError::LimitExceeded)?;
        if count > expected.saturating_sub(samples.len()) {
            return Err(AnalysisError::Invalid("decoded frame count exceeds source"));
        }
        let mut buffer = SampleBuffer::<f32>::new(decoded.frames() as u64, *decoded.spec());
        buffer.copy_interleaved_ref(decoded);
        samples.extend_from_slice(buffer.samples());
        progress.report(
            (samples.len() / metadata.channels() as usize) as u64,
            metadata.frames().get() as u64,
        );
    }
    cancel.check()?;
    if samples.len() != expected {
        return Err(AnalysisError::Invalid(
            "truncated PCM or mismatched source frame count",
        ));
    }
    Pcm::new(metadata, samples).map_err(map_error)
}
impl Analyzer for DecodeAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "audio.decode".into(),
            version: "1".into(),
            provider: "symphonia-0.5.5/wav-pcm".into(),
            model: None,
            kind: AnalysisKind::Decode,
            deterministic: true,
            hardware: HardwareRequirements {
                cpu_threads: 1,
                exclusive_gpu: false,
            },
        }
    }
    fn dependencies(&self) -> Vec<AnalysisKind> {
        vec![]
    }
    fn supports(&self, ctx: &AnalysisContext) -> SupportLevel {
        if ctx.settings.is_empty() && (1..=2).contains(&ctx.source.metadata().channels()) {
            SupportLevel::Supported
        } else {
            SupportLevel::Unsupported
        }
    }
    fn validate_inputs(
        &self,
        ctx: &AnalysisContext,
        cancel: &CancellationToken,
    ) -> Result<(), AnalysisError> {
        let bytes = (ctx.source.metadata().frames().get() as u64)
            .checked_mul(ctx.source.metadata().channels() as u64)
            .and_then(|n| n.checked_mul(4));
        if bytes.is_none_or(|n| n > MAX_PCM_BYTES) {
            return Err(AnalysisError::LimitExceeded);
        }
        verify_source(&ctx.source, cancel).map_err(audio_error)
    }
    fn run(
        &self,
        ctx: AnalysisContext,
        cancel: CancellationToken,
        progress: ProgressSink,
    ) -> Result<AnalysisArtifact, AnalysisError> {
        if self.supports(&ctx) != SupportLevel::Supported || !ctx.dependencies.is_empty() {
            return Err(AnalysisError::Unsupported);
        }
        let pcm = decode_pcm(&ctx, &cancel, &progress)?;
        let confidence=Confidence::new(None,ConfidenceKind::Measurement,"Decoded samples; probability confidence is not applicable. No clipping, mixing or resampling.".into()).map_err(map_error)?;
        AnalysisArtifact::new(
            &self.descriptor(),
            &ctx,
            ArtifactPayload::Pcm(pcm),
            confidence,
            &cancel,
        )
    }
}
