use vocal_analysis_api::{AnalysisArtifact, AnalysisError, ArtifactPayload};
use vocal_domain::{
    audio::{AudioMetadata, ContentHash},
    time::{Samples, Seconds},
};

pub const MAX_VIEWPORT_PIXELS: u32 = 4096;

#[derive(Debug, Clone, Copy)]
pub struct WaveformViewport {
    start: Seconds,
    end: Seconds,
    pixel_width: u32,
}
impl WaveformViewport {
    pub fn new(start: Seconds, end: Seconds, pixel_width: u32) -> Result<Self, AnalysisError> {
        if end <= start || pixel_width == 0 || pixel_width > MAX_VIEWPORT_PIXELS {
            return Err(AnalysisError::Invalid(
                "viewport requires increasing time and 1..4096 pixels",
            ));
        }
        Ok(Self {
            start,
            end,
            pixel_width,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaveformPoint {
    /// Full aggregate block span, half-open. The renderer clips to its viewport.
    pub start: Samples,
    pub end: Samples,
    pub min: f32,
    pub max: f32,
    pub rms: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformSlice {
    /// Links this read-only projection to its parent's confidence and provenance.
    pub artifact_hash: ContentHash,
    pub metadata: AudioMetadata,
    pub frames_per_block: u64,
    pub channels: Vec<Vec<WaveformPoint>>,
}

/// Queries an already validated artifact. No PCM scan or analyzer/cache mutation.
/// At most pixel_width + 1 complete aggregate blocks are returned per channel.
pub fn query_waveform(
    artifact: &AnalysisArtifact,
    viewport: WaveformViewport,
) -> Result<WaveformSlice, AnalysisError> {
    let ArtifactPayload::Waveform(waveform) = artifact.payload() else {
        return Err(AnalysisError::Invalid(
            "waveform query requires a waveform artifact",
        ));
    };
    let metadata = waveform.metadata;
    let duration = metadata.duration().get();
    let total = metadata.frames().get() as u64;
    let start = sample_boundary(
        viewport.start.get().clamp(0.0, duration),
        metadata.sample_rate(),
        false,
    )
    .min(total);
    let end = sample_boundary(
        viewport.end.get().clamp(0.0, duration),
        metadata.sample_rate(),
        true,
    )
    .min(total);
    if end <= start {
        return Ok(WaveformSlice {
            artifact_hash: artifact.artifact_hash(),
            metadata,
            frames_per_block: waveform.levels[0].frames_per_block,
            channels: vec![Vec::new(); metadata.channels() as usize],
        });
    }
    let desired = (end - start).div_ceil(viewport.pixel_width as u64);
    let level = waveform
        .levels
        .iter()
        .find(|level| level.frames_per_block >= desired)
        .ok_or(AnalysisError::Invalid(
            "pyramid lacks requested coarse resolution",
        ))?;
    let width = level.frames_per_block;
    let first = (start / width) as usize;
    let stop = end.div_ceil(width) as usize;
    if stop - first > viewport.pixel_width as usize + 1 {
        return Err(AnalysisError::Invalid("viewport output bound exceeded"));
    }
    let channels = level
        .channels
        .iter()
        .map(|channel| {
            channel[first..stop]
                .iter()
                .enumerate()
                .map(|(offset, peak)| {
                    let block_start = (first + offset) as u64 * width;
                    WaveformPoint {
                        start: Samples::new(block_start as i64),
                        end: Samples::new((block_start + peak.frames as u64) as i64),
                        min: peak.min,
                        max: peak.max,
                        rms: peak.rms,
                    }
                })
                .collect()
        })
        .collect();
    Ok(WaveformSlice {
        artifact_hash: artifact.artifact_hash(),
        metadata,
        frames_per_block: width,
        channels,
    })
}

fn sample_boundary(seconds: f64, rate: u32, ceil: bool) -> u64 {
    let raw = seconds * rate as f64;
    // Stabilize sec -> sample conversions at exact sample boundaries (within four ULP-scale epsilons).
    let nearest = raw.round();
    let raw = if (raw - nearest).abs() <= 4.0 * f64::EPSILON * raw.abs().max(1.0) {
        nearest
    } else {
        raw
    };
    if ceil {
        raw.ceil() as u64
    } else {
        raw.floor() as u64
    }
}
