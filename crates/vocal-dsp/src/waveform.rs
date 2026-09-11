use vocal_analysis_api::*;
use vocal_domain::{
    analysis::{Confidence, ConfidenceKind},
    signal::{Peak, WaveformLevel, WaveformPyramid},
};

pub struct WaveformAnalyzer;
fn base_frames(ctx: &AnalysisContext) -> Option<u64> {
    if ctx.settings.keys().any(|k| k != "base_frames") {
        return None;
    }
    let text = ctx
        .settings
        .get("base_frames")
        .map(String::as_str)
        .unwrap_or("256");
    let value = text.parse::<u64>().ok()?;
    (value.to_string() == text && (256..=65536).contains(&value) && value.is_power_of_two())
        .then_some(value)
}
impl Analyzer for WaveformAnalyzer {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            id: "dsp.waveform-peaks".into(),
            version: "1".into(),
            provider: "rust-min-max-rms".into(),
            model: None,
            kind: AnalysisKind::WaveformPeaks,
            deterministic: true,
            hardware: HardwareRequirements {
                cpu_threads: 1,
                exclusive_gpu: false,
            },
        }
    }
    fn dependencies(&self) -> Vec<AnalysisKind> {
        vec![AnalysisKind::Decode]
    }
    fn supports(&self, ctx: &AnalysisContext) -> SupportLevel {
        if base_frames(ctx).is_some() {
            SupportLevel::Supported
        } else {
            SupportLevel::Unsupported
        }
    }
    fn normalized_settings(
        &self,
        ctx: &AnalysisContext,
    ) -> Result<std::collections::BTreeMap<String, String>, AnalysisError> {
        Ok(std::collections::BTreeMap::from([(
            "base_frames".into(),
            base_frames(ctx)
                .ok_or(AnalysisError::Unsupported)?
                .to_string(),
        )]))
    }
    fn run(
        &self,
        mut ctx: AnalysisContext,
        cancel: CancellationToken,
        progress: ProgressSink,
    ) -> Result<AnalysisArtifact, AnalysisError> {
        cancel.check()?;
        ctx.settings = self.normalized_settings(&ctx)?;
        let width = base_frames(&ctx).ok_or(AnalysisError::Unsupported)?;
        if ctx.dependencies.len() != 1 {
            return Err(AnalysisError::Invalid(
                "waveform requires one Decode artifact",
            ));
        }
        let dep = &ctx.dependencies[0];
        if dep.kind() != AnalysisKind::Decode
            || dep.provenance().source_hash != ctx.source.content_hash()
        {
            return Err(AnalysisError::Invalid("wrong waveform dependency"));
        }
        let ArtifactPayload::Pcm(pcm) = dep.payload() else {
            return Err(AnalysisError::Invalid("expected PCM"));
        };
        if pcm.metadata() != ctx.source.metadata() {
            return Err(AnalysisError::Invalid("PCM metadata mismatch"));
        }
        let channel_count = pcm.metadata().channels() as usize;
        let mut first = WaveformLevel {
            frames_per_block: width,
            channels: vec![Vec::new(); channel_count],
        };
        for (i, chunk) in pcm
            .samples()
            .chunks(width as usize * channel_count)
            .enumerate()
        {
            cancel.check()?;
            let frames = chunk.len() / channel_count;
            for channel in 0..channel_count {
                let mut min = f32::INFINITY;
                let mut max = f32::NEG_INFINITY;
                let mut squares = 0.0;
                for frame in chunk.chunks_exact(channel_count) {
                    let value = frame[channel];
                    min = min.min(value);
                    max = max.max(value);
                    squares += (value as f64).powi(2);
                }
                first.channels[channel].push(Peak {
                    frames: frames as u32,
                    min,
                    max,
                    rms: (squares / frames as f64).sqrt(),
                });
            }
            progress.report(
                ((i + 1) as u64 * width).min(pcm.metadata().frames().get() as u64),
                pcm.metadata().frames().get() as u64,
            );
        }
        let mut levels = vec![first];
        while levels.last().unwrap().channels[0].len() > 1 {
            cancel.check()?;
            let previous = levels.last().unwrap();
            let mut next = WaveformLevel {
                frames_per_block: previous.frames_per_block * 2,
                channels: Vec::new(),
            };
            for channel in &previous.channels {
                let mut peaks = Vec::new();
                for (i, pair) in channel.chunks(2).enumerate() {
                    if i % 4096 == 0 {
                        cancel.check()?;
                    }
                    let frames = pair.iter().map(|p| p.frames).sum::<u32>();
                    peaks.push(Peak {
                        frames,
                        min: pair.iter().map(|p| p.min).fold(f32::INFINITY, f32::min),
                        max: pair.iter().map(|p| p.max).fold(f32::NEG_INFINITY, f32::max),
                        rms: (pair
                            .iter()
                            .map(|p| p.rms * p.rms * p.frames as f64)
                            .sum::<f64>()
                            / frames as f64)
                            .sqrt(),
                    });
                }
                next.channels.push(peaks);
            }
            levels.push(next);
        }
        let confidence = Confidence::new(
            None,
            ConfidenceKind::Measurement,
            "Per-channel extrema and frame-weighted RMS; probability score not applicable.".into(),
        )
        .map_err(|_| AnalysisError::Invalid("confidence"))?;
        AnalysisArtifact::new(
            &self.descriptor(),
            &ctx,
            ArtifactPayload::Waveform(WaveformPyramid {
                metadata: pcm.metadata(),
                levels,
            }),
            confidence,
            &cancel,
        )
    }
}
