use std::path::PathBuf;
use vocal_analysis_api::*;
use vocal_analysis_runtime::AnalysisRuntime;
use vocal_audio::{DecodeAnalyzer, WavImporter};
use vocal_dsp::WaveformAnalyzer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/stereo-48000.wav")
        });
    let cancel = CancellationToken::default();
    let source = WavImporter::import_linked(path, &cancel)?;
    let mut runtime = AnalysisRuntime::new(128 * 1024 * 1024);
    let ctx = AnalysisContext::new(source);
    let decode = runtime.execute(
        &DecodeAnalyzer,
        ctx.clone(),
        cancel.clone(),
        ProgressSink::default(),
    )?;
    let mut wave = ctx;
    wave.dependencies.push(decode.artifact);
    let first = runtime.execute(
        &WaveformAnalyzer,
        wave.clone(),
        cancel.clone(),
        ProgressSink::default(),
    )?;
    let cached = runtime.execute(&WaveformAnalyzer, wave, cancel, ProgressSink::default())?;
    assert!(cached.cache_hit);
    let ArtifactPayload::Waveform(pyramid) = first.artifact.payload() else {
        unreachable!()
    };
    println!(
        "Decode -> WaveformPeaks: {} channels, {} frames, {} levels",
        pyramid.metadata.channels(),
        pyramid.metadata.frames().get(),
        pyramid.levels.len()
    );
    for level in &pyramid.levels {
        println!(
            "{} frames/block: {} blocks/channel",
            level.frames_per_block,
            level.channels[0].len()
        );
    }
    println!(
        "provider: {}; confidence: {:?}; score: {:?}",
        first.artifact.provenance().provider,
        first.artifact.confidence().kind(),
        first.artifact.confidence().score()
    );
    println!(
        "artifact: {}; cache hit: {}",
        first.artifact.artifact_hash(),
        cached.cache_hit
    );
    let view = vocal_dsp::WaveformViewport::new(
        vocal_domain::time::Seconds::new(0.0)?,
        pyramid.metadata.duration(),
        2,
    )?;
    let slice = vocal_dsp::query_waveform(&first.artifact, view)?;
    println!(
        "2-pixel viewport: {} blocks/channel at {} frames/block",
        slice.channels[0].len(),
        slice.frames_per_block
    );
    Ok(())
}
