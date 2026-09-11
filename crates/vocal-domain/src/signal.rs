use crate::audio::{AudioDomainError, AudioMetadata};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    metadata: AudioMetadata,
    samples: Arc<[f32]>,
}
impl Pcm {
    /// Interleaved original-rate samples. No clipping or channel conversion.
    pub fn new(metadata: AudioMetadata, samples: Vec<f32>) -> Result<Self, AudioDomainError> {
        let expected = (metadata.frames().get() as u64).checked_mul(metadata.channels() as u64);
        if expected != Some(samples.len() as u64) || samples.iter().any(|s| !s.is_finite()) {
            return Err(AudioDomainError("invalid PCM shape or nonfinite sample"));
        }
        Ok(Self {
            metadata,
            samples: samples.into(),
        })
    }
    pub fn metadata(&self) -> AudioMetadata {
        self.metadata
    }
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Peak {
    pub frames: u32,
    pub min: f32,
    pub max: f32,
    pub rms: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformLevel {
    pub frames_per_block: u64,
    /// One independent peak sequence per source channel.
    pub channels: Vec<Vec<Peak>>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformPyramid {
    pub metadata: AudioMetadata,
    pub levels: Vec<WaveformLevel>,
}
