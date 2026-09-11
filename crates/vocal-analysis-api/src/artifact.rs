use crate::{
    cache_key, contract::HashBuilder, settings_hash, AnalysisContext, AnalysisError, AnalysisKind,
    AnalyzerDescriptor, CancellationToken,
};
use std::time::SystemTime;
use vocal_domain::{
    analysis::{Confidence, Provenance},
    audio::ContentHash,
    signal::{Pcm, WaveformPyramid},
};

#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactPayload {
    Pcm(Pcm),
    Waveform(WaveformPyramid),
}
#[derive(Debug, Clone)]
pub struct AnalysisArtifact {
    kind: AnalysisKind,
    cache_key: ContentHash,
    artifact_hash: ContentHash,
    provenance: Provenance,
    confidence: Confidence,
    payload: ArtifactPayload,
}
impl AnalysisArtifact {
    pub fn new(
        descriptor: &AnalyzerDescriptor,
        ctx: &AnalysisContext,
        payload: ArtifactPayload,
        confidence: Confidence,
        cancel: &CancellationToken,
    ) -> Result<Self, AnalysisError> {
        cancel.check()?;
        if descriptor.id.is_empty()
            || descriptor.version.is_empty()
            || descriptor.provider.is_empty()
            || descriptor
                .model
                .as_ref()
                .is_some_and(|m| m.id.is_empty() || m.version.is_empty())
        {
            return Err(AnalysisError::Invalid("incomplete provider identity"));
        }
        let key = cache_key(descriptor, ctx);
        let mut h = HashBuilder::new("analysis-artifact-v1");
        h.bytes(&key.0);
        h.bytes(&[confidence.kind() as u8]);
        h.bytes(
            &confidence
                .score()
                .map(f32::to_le_bytes)
                .unwrap_or([0xff; 4]),
        );
        h.bytes(confidence.explanation().as_bytes());
        match &payload {
            ArtifactPayload::Pcm(pcm) => {
                if descriptor.kind != AnalysisKind::Decode
                    || pcm.metadata() != ctx.source.metadata()
                    || pcm.samples().len() as u64 * 4 > crate::MAX_PCM_BYTES
                {
                    return Err(AnalysisError::Invalid(
                        "PCM artifact does not match source/kind/limit",
                    ));
                }
                for chunk in pcm.samples().chunks(16384) {
                    cancel.check()?;
                    for value in chunk {
                        h.bytes(&value.to_le_bytes());
                    }
                }
            }
            ArtifactPayload::Waveform(w) => {
                if descriptor.kind != AnalysisKind::WaveformPeaks
                    || w.metadata != ctx.source.metadata()
                    || w.levels.is_empty()
                {
                    return Err(AnalysisError::Invalid("invalid waveform metadata"));
                }
                let mut previous = 0_u64;
                for level in &w.levels {
                    cancel.check()?;
                    let width = level.frames_per_block;
                    if width == 0
                        || (previous != 0 && Some(width) != previous.checked_mul(2))
                        || level.channels.len() != w.metadata.channels() as usize
                    {
                        return Err(AnalysisError::Invalid("invalid pyramid structure"));
                    }
                    previous = width;
                    h.bytes(&width.to_le_bytes());
                    let total = w.metadata.frames().get() as u64;
                    for channel in &level.channels {
                        if channel.len() as u64 != total.div_ceil(width) {
                            return Err(AnalysisError::Invalid("invalid peak count"));
                        }
                        for (i, p) in channel.iter().enumerate() {
                            if i % 4096 == 0 {
                                cancel.check()?;
                            }
                            if p.frames as u64 != width.min(total - i as u64 * width)
                                || !p.min.is_finite()
                                || !p.max.is_finite()
                                || !p.rms.is_finite()
                                || p.min > p.max
                                || p.rms < 0.0
                                || p.rms > (p.min.abs().max(p.max.abs()) as f64) * (1.0 + 1e-12)
                            {
                                return Err(AnalysisError::Invalid("invalid peak values"));
                            }
                            h.bytes(&p.frames.to_le_bytes());
                            h.bytes(&p.min.to_le_bytes());
                            h.bytes(&p.max.to_le_bytes());
                            h.bytes(&p.rms.to_le_bytes());
                        }
                    }
                }
            }
        }
        cancel.check()?;
        Ok(Self {
            kind: descriptor.kind,
            cache_key: key,
            artifact_hash: h.finish(),
            confidence,
            payload,
            provenance: Provenance {
                analyzer_id: descriptor.id.clone(),
                analyzer_version: descriptor.version.clone(),
                provider: descriptor.provider.clone(),
                model: descriptor.model.clone(),
                settings: ctx.settings.clone(),
                settings_hash: settings_hash(&ctx.settings),
                source_hash: ctx.source.content_hash(),
                dependency_hashes: ctx.dependencies.iter().map(|a| a.artifact_hash()).collect(),
                created_at: SystemTime::now(),
                runtime: format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            },
        })
    }
    pub fn kind(&self) -> AnalysisKind {
        self.kind
    }
    pub fn cache_key(&self) -> ContentHash {
        self.cache_key
    }
    pub fn artifact_hash(&self) -> ContentHash {
        self.artifact_hash
    }
    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
    pub fn confidence(&self) -> &Confidence {
        &self.confidence
    }
    pub fn payload(&self) -> &ArtifactPayload {
        &self.payload
    }
    pub fn payload_bytes(&self) -> usize {
        match &self.payload {
            ArtifactPayload::Pcm(p) => p.samples().len() * 4,
            ArtifactPayload::Waveform(w) => w
                .levels
                .iter()
                .flat_map(|l| &l.channels)
                .map(|c| c.len() * std::mem::size_of::<vocal_domain::signal::Peak>())
                .sum(),
        }
    }
}
