use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use vocal_analysis_api::*;
use vocal_analysis_runtime::AnalysisRuntime;
use vocal_audio::{DecodeAnalyzer, WavImporter};
use vocal_domain::{
    analysis::{Confidence, ConfidenceKind, ModelIdentity},
    audio::{AudioMetadata, AudioSource, ContentHash},
    identity::EntityId,
    signal::Pcm,
    time::Samples,
};
use vocal_dsp::WaveformAnalyzer;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/audio")
        .join(name)
}
fn context(name: &str) -> AnalysisContext {
    AnalysisContext::new(
        WavImporter::import_linked(fixture(name), &CancellationToken::default()).unwrap(),
    )
}
fn execute(
    runtime: &mut AnalysisRuntime,
    analyzer: &dyn Analyzer,
    ctx: AnalysisContext,
) -> vocal_analysis_runtime::Execution {
    runtime
        .execute(
            analyzer,
            ctx,
            CancellationToken::default(),
            ProgressSink::default(),
        )
        .unwrap()
}

#[test]
fn pcm_matches_known_fixture_samples_without_channel_conversion() {
    for name in ["mono-48000.wav", "stereo-48000.wav"] {
        let ctx = context(name);
        let original = ctx.source.clone();
        let mut runtime = AnalysisRuntime::new(1024 * 1024);
        let result = execute(&mut runtime, &DecodeAnalyzer, ctx);
        let ArtifactPayload::Pcm(pcm) = result.artifact.payload() else {
            panic!("PCM required")
        };
        assert_eq!(pcm.metadata(), original.metadata());
        for (i, &actual) in pcm.samples().iter().enumerate() {
            assert_eq!(actual, ((i % 32) as f32 * 128.0 - 2048.0) / 32768.0);
        }
        let p = result.artifact.provenance();
        assert_eq!(p.source_hash, original.content_hash());
        assert_eq!(p.analyzer_id, "audio.decode");
        assert!(!p.analyzer_version.is_empty() && !p.provider.is_empty() && !p.runtime.is_empty());
        assert_eq!(p.settings_hash, settings_hash(&p.settings));
        assert!(p.dependency_hashes.is_empty() && p.model.is_none());
        assert_eq!(
            result.artifact.confidence().kind(),
            ConfidenceKind::Measurement
        );
        assert_eq!(result.artifact.confidence().score(), None);
    }
}

#[test]
fn two_node_chain_caches_normalized_defaults_and_records_dependencies() {
    let ctx = context("stereo-48000.wav");
    let mut runtime = AnalysisRuntime::new(1024 * 1024);
    let decode = execute(&mut runtime, &DecodeAnalyzer, ctx.clone());
    assert!(!decode.cache_hit);
    assert!(execute(&mut runtime, &DecodeAnalyzer, ctx.clone()).cache_hit);
    let mut wave = ctx;
    wave.dependencies = vec![decode.artifact.clone()];
    let first = execute(&mut runtime, &WaveformAnalyzer, wave.clone());
    wave.settings.insert("base_frames".into(), "256".into());
    let second = execute(&mut runtime, &WaveformAnalyzer, wave.clone());
    assert!(second.cache_hit && Arc::ptr_eq(&first.artifact, &second.artifact));
    assert_eq!(
        first.artifact.provenance().dependency_hashes,
        vec![decode.artifact.artifact_hash()]
    );
    assert_eq!(first.artifact.provenance().settings["base_frames"], "256");
    wave.settings.insert("base_frames".into(), "512".into());
    let changed = execute(&mut runtime, &WaveformAnalyzer, wave);
    assert!(!changed.cache_hit);
    assert_ne!(changed.artifact.cache_key(), first.artifact.cache_key());
    let ArtifactPayload::Waveform(w) = first.artifact.payload() else {
        panic!("waveform")
    };
    assert_eq!(
        w.levels
            .iter()
            .map(|l| l.frames_per_block)
            .collect::<Vec<_>>(),
        vec![256, 512, 1024]
    );
    for level in &w.levels {
        assert_eq!(level.channels.len(), 2);
    }
}

#[test]
fn weighted_tail_rms_and_extrema_are_correct() {
    let mut ctx = context("mono-48000.wav");
    ctx.source = AudioSource::new(
        ctx.source.id(),
        ctx.source.uri().clone(),
        ctx.source.content_hash(),
        ctx.source.size_bytes(),
        AudioMetadata::new(48000, 1, Samples::new(257)).unwrap(),
    )
    .unwrap();
    let mut samples = vec![0.0; 257];
    samples[256] = 1.0;
    let confidence = Confidence::new(
        None,
        ConfidenceKind::Measurement,
        "Synthetic numeric fixture".into(),
    )
    .unwrap();
    let pcm = Pcm::new(ctx.source.metadata(), samples).unwrap();
    let artifact = AnalysisArtifact::new(
        &DecodeAnalyzer.descriptor(),
        &ctx,
        ArtifactPayload::Pcm(pcm),
        confidence,
        &CancellationToken::default(),
    )
    .unwrap();
    ctx.dependencies.push(Arc::new(artifact));
    let result = execute(&mut AnalysisRuntime::new(100000), &WaveformAnalyzer, ctx);
    let ArtifactPayload::Waveform(w) = result.artifact.payload() else {
        panic!("waveform")
    };
    assert_eq!(w.levels[0].channels[0][0].frames, 256);
    assert_eq!(w.levels[0].channels[0][1].frames, 1);
    let top = &w.levels[1].channels[0][0];
    assert_eq!((top.frames, top.min, top.max), (257, 0.0, 1.0));
    assert!((top.rms - (1.0_f64 / 257.0).sqrt()).abs() < 1e-12);
}

#[test]
fn missing_extra_wrong_source_dependencies_and_unknown_settings_fail() {
    let ctx = context("mono-48000.wav");
    let mut runtime = AnalysisRuntime::new(100000);
    assert!(runtime
        .execute(
            &WaveformAnalyzer,
            ctx.clone(),
            CancellationToken::default(),
            ProgressSink::default()
        )
        .is_err());
    let decode = execute(&mut runtime, &DecodeAnalyzer, ctx.clone()).artifact;
    let mut extra = ctx.clone();
    extra.dependencies = vec![decode.clone()];
    assert!(runtime
        .execute(
            &DecodeAnalyzer,
            extra,
            CancellationToken::default(),
            ProgressSink::default()
        )
        .is_err());
    let mut wrong = context("stereo-48000.wav");
    wrong.dependencies = vec![decode.clone()];
    assert!(runtime
        .execute(
            &WaveformAnalyzer,
            wrong,
            CancellationToken::default(),
            ProgressSink::default()
        )
        .is_err());
    for value in ["0", "0256", "257", "131072"] {
        let mut bad = ctx.clone();
        bad.dependencies = vec![decode.clone()];
        bad.settings.insert("base_frames".into(), value.into());
        assert!(runtime
            .execute(
                &WaveformAnalyzer,
                bad,
                CancellationToken::default(),
                ProgressSink::default()
            )
            .is_err());
    }
    let mut bad = ctx;
    bad.settings.insert("resample".into(), "16000".into());
    assert!(runtime
        .execute(
            &DecodeAnalyzer,
            bad,
            CancellationToken::default(),
            ProgressSink::default()
        )
        .is_err());
}

#[test]
fn source_change_is_detected_even_before_a_cache_hit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vocal.wav");
    fs::copy(fixture("mono-48000.wav"), &path).unwrap();
    let ctx = AnalysisContext::new(
        WavImporter::import_linked(&path, &CancellationToken::default()).unwrap(),
    );
    let mut runtime = AnalysisRuntime::new(100000);
    execute(&mut runtime, &DecodeAnalyzer, ctx.clone());
    let mut bytes = fs::read(&path).unwrap();
    bytes[44] ^= 1;
    fs::write(&path, bytes).unwrap();
    assert!(matches!(
        runtime.execute(
            &DecodeAnalyzer,
            ctx,
            CancellationToken::default(),
            ProgressSink::default()
        ),
        Err(AnalysisError::SourceChanged)
    ));
    assert_eq!(runtime.cached_entries(), 1); // Old immutable content still has a valid artifact.
}

#[test]
fn cancellation_before_during_decode_and_during_waveform_does_not_publish() {
    let ctx = context("mono-48000.wav");
    let mut runtime = AnalysisRuntime::new(100000);
    let token = CancellationToken::default();
    token.cancel();
    assert!(matches!(
        runtime.execute(&DecodeAnalyzer, ctx.clone(), token, ProgressSink::default()),
        Err(AnalysisError::Cancelled)
    ));
    let token = CancellationToken::default();
    let trigger = token.clone();
    let progress = ProgressSink::new(move |p| {
        if p.completed > 0 {
            trigger.cancel();
        }
    });
    assert!(matches!(
        runtime.execute(&DecodeAnalyzer, ctx.clone(), token, progress),
        Err(AnalysisError::Cancelled)
    ));
    assert_eq!(runtime.cached_entries(), 0);
    let decode = execute(&mut runtime, &DecodeAnalyzer, ctx.clone()).artifact;
    let mut wave = ctx;
    wave.dependencies.push(decode);
    let token = CancellationToken::default();
    let trigger = token.clone();
    assert!(matches!(
        runtime.execute(
            &WaveformAnalyzer,
            wave,
            token,
            ProgressSink::new(move |_| trigger.cancel())
        ),
        Err(AnalysisError::Cancelled)
    ));
    assert_eq!(runtime.cached_entries(), 1);
}

#[test]
fn limits_and_truncated_payload_fail_without_publishing() {
    let mut ctx = context("mono-48000.wav");
    ctx.source = AudioSource::new(
        ctx.source.id(),
        ctx.source.uri().clone(),
        ctx.source.content_hash(),
        ctx.source.size_bytes(),
        AudioMetadata::new(48000, 1, Samples::new(MAX_PCM_BYTES as i64 / 4 + 1)).unwrap(),
    )
    .unwrap();
    let mut runtime = AnalysisRuntime::new(100000);
    assert!(matches!(
        runtime.execute(
            &DecodeAnalyzer,
            ctx,
            CancellationToken::default(),
            ProgressSink::default()
        ),
        Err(AnalysisError::LimitExceeded)
    ));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("truncated.wav");
    let mut bytes = fs::read(fixture("mono-48000.wav")).unwrap();
    bytes.truncate(500);
    let riff_size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
    let data_size = (bytes.len() - 44) as u32;
    bytes[40..44].copy_from_slice(&data_size.to_le_bytes());
    fs::write(&path, bytes).unwrap();
    // A valid shorter WAV paired with stale project frame metadata must fail actual decoding.
    let source = WavImporter::import_linked(&path, &CancellationToken::default()).unwrap();
    let source = AudioSource::new(
        source.id(),
        source.uri().clone(),
        source.content_hash(),
        source.size_bytes(),
        AudioMetadata::new(48000, 1, Samples::new(480)).unwrap(),
    )
    .unwrap();
    assert!(runtime
        .execute(
            &DecodeAnalyzer,
            AnalysisContext::new(source),
            CancellationToken::default(),
            ProgressSink::default()
        )
        .is_err());
    assert_eq!(runtime.cached_entries(), 0);
}

struct AlternateDecode {
    version: &'static str,
    calls: AtomicUsize,
    model: Option<ModelIdentity>,
    cancel_after: bool,
}
impl Analyzer for AlternateDecode {
    fn descriptor(&self) -> AnalyzerDescriptor {
        AnalyzerDescriptor {
            version: self.version.into(),
            provider: "test-alternate-adapter".into(),
            model: self.model.clone(),
            ..DecodeAnalyzer.descriptor()
        }
    }
    fn dependencies(&self) -> Vec<AnalysisKind> {
        vec![]
    }
    fn supports(&self, ctx: &AnalysisContext) -> SupportLevel {
        DecodeAnalyzer.supports(ctx)
    }
    fn validate_inputs(
        &self,
        ctx: &AnalysisContext,
        cancel: &CancellationToken,
    ) -> Result<(), AnalysisError> {
        DecodeAnalyzer.validate_inputs(ctx, cancel)
    }
    fn run(
        &self,
        ctx: AnalysisContext,
        cancel: CancellationToken,
        progress: ProgressSink,
    ) -> Result<AnalysisArtifact, AnalysisError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let base = DecodeAnalyzer.run(ctx.clone(), cancel.clone(), progress)?;
        let artifact = AnalysisArtifact::new(
            &self.descriptor(),
            &ctx,
            base.payload().clone(),
            base.confidence().clone(),
            &cancel,
        )?;
        if self.cancel_after {
            cancel.cancel();
        }
        Ok(artifact)
    }
}
fn alternate(version: &'static str) -> AlternateDecode {
    AlternateDecode {
        version,
        calls: AtomicUsize::new(0),
        model: None,
        cancel_after: false,
    }
}

#[test]
fn interchangeable_providers_and_versions_invalidate_downstream_cache() {
    let ctx = context("mono-48000.wav");
    let mut runtime = AnalysisRuntime::new(100000);
    let one = alternate("1");
    let two = alternate("2");
    let a = execute(&mut runtime, &one, ctx.clone()).artifact;
    assert!(execute(&mut runtime, &one, ctx.clone()).cache_hit);
    assert_eq!(one.calls.load(Ordering::Relaxed), 1);
    let b = execute(&mut runtime, &two, ctx.clone()).artifact;
    assert_eq!(a.payload(), b.payload());
    assert_ne!(a.artifact_hash(), b.artifact_hash());
    let mut first = ctx.clone();
    first.dependencies = vec![a];
    let wa = execute(&mut runtime, &WaveformAnalyzer, first).artifact;
    let mut second = ctx.clone();
    second.dependencies = vec![b];
    let wb = execute(&mut runtime, &WaveformAnalyzer, second);
    assert!(!wb.cache_hit);
    assert_ne!(wa.cache_key(), wb.artifact.cache_key());
    let mut model = alternate("2");
    model.model = Some(ModelIdentity {
        id: "fixture-model".into(),
        version: "1".into(),
        hash: ContentHash([1; 32]),
    });
    let ma = execute(&mut runtime, &model, ctx.clone()).artifact;
    model.model.as_mut().unwrap().hash = ContentHash([2; 32]);
    let mb = execute(&mut runtime, &model, ctx).artifact;
    assert_ne!(ma.cache_key(), mb.cache_key());
    assert_eq!(mb.provenance().model, model.model);
}

#[test]
fn cancellation_at_provider_return_and_cache_capacity_are_enforced() {
    let ctx = context("mono-48000.wav");
    let mut runtime = AnalysisRuntime::new(2000);
    let mut cancelled = alternate("1");
    cancelled.cancel_after = true;
    assert!(matches!(
        runtime.execute(
            &cancelled,
            ctx.clone(),
            CancellationToken::default(),
            ProgressSink::default()
        ),
        Err(AnalysisError::Cancelled)
    ));
    assert_eq!(runtime.cached_entries(), 0);
    execute(&mut runtime, &DecodeAnalyzer, ctx.clone());
    execute(&mut runtime, &alternate("2"), ctx.clone());
    assert_eq!(runtime.cached_entries(), 1);
    assert!(runtime.cached_payload_bytes() <= 2000);
    assert!(!execute(&mut runtime, &DecodeAnalyzer, ctx.clone()).cache_hit);
    let mut no_cache = AnalysisRuntime::new(0);
    execute(&mut no_cache, &DecodeAnalyzer, ctx);
    assert_eq!(no_cache.cached_entries(), 0);
}

#[test]
fn cache_identity_ignores_entity_id_but_keeps_source_metadata_and_hash() {
    let ctx = context("mono-48000.wav");
    let mut renamed = ctx.clone();
    renamed.source = AudioSource::new(
        EntityId::new(),
        ctx.source.uri().clone(),
        ctx.source.content_hash(),
        ctx.source.size_bytes(),
        ctx.source.metadata(),
    )
    .unwrap();
    assert_eq!(
        cache_key(&DecodeAnalyzer.descriptor(), &ctx),
        cache_key(&DecodeAnalyzer.descriptor(), &renamed)
    );
    renamed.source = AudioSource::new(
        EntityId::new(),
        ctx.source.uri().clone(),
        ContentHash([8; 32]),
        ctx.source.size_bytes(),
        ctx.source.metadata(),
    )
    .unwrap();
    assert_ne!(
        cache_key(&DecodeAnalyzer.descriptor(), &ctx),
        cache_key(&DecodeAnalyzer.descriptor(), &renamed)
    );
}

#[test]
fn invalid_confidence_pcm_and_waveform_are_rejected_at_artifact_boundary() {
    for score in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(Confidence::new(Some(score), ConfidenceKind::Heuristic, "fixture".into()).is_err());
    }
    assert!(Confidence::new(
        Some(1.0),
        ConfidenceKind::Measurement,
        "not a probability".into()
    )
    .is_err());
    let ctx = context("mono-48000.wav");
    assert!(Pcm::new(ctx.source.metadata(), vec![f32::NAN; 480]).is_err());
    assert!(Pcm::new(ctx.source.metadata(), vec![0.0; 479]).is_err());
    let mut runtime = AnalysisRuntime::new(100000);
    let decode = execute(&mut runtime, &DecodeAnalyzer, ctx.clone()).artifact;
    let mut wave = ctx;
    wave.dependencies.push(decode);
    let artifact = execute(&mut runtime, &WaveformAnalyzer, wave.clone()).artifact;
    let ArtifactPayload::Waveform(mut invalid) = artifact.payload().clone() else {
        panic!("waveform")
    };
    invalid.levels[0].channels[0][0].rms = f64::NAN;
    assert!(AnalysisArtifact::new(
        &WaveformAnalyzer.descriptor(),
        &wave,
        ArtifactPayload::Waveform(invalid),
        artifact.confidence().clone(),
        &CancellationToken::default()
    )
    .is_err());
}

#[test]
fn deterministic_artifact_hash_excludes_timestamp_and_bad_provider_results_fail() {
    let ctx = context("mono-48000.wav");
    let mut runtime = AnalysisRuntime::new(0);
    let first = execute(&mut runtime, &DecodeAnalyzer, ctx.clone()).artifact;
    let second = execute(&mut runtime, &DecodeAnalyzer, ctx.clone()).artifact;
    assert_eq!(first.artifact_hash(), second.artifact_hash());
    struct BadProvider;
    impl Analyzer for BadProvider {
        fn descriptor(&self) -> AnalyzerDescriptor {
            AnalyzerDescriptor {
                version: "wrong".into(),
                ..DecodeAnalyzer.descriptor()
            }
        }
        fn dependencies(&self) -> Vec<AnalysisKind> {
            vec![]
        }
        fn supports(&self, _: &AnalysisContext) -> SupportLevel {
            SupportLevel::Supported
        }
        fn run(
            &self,
            ctx: AnalysisContext,
            cancel: CancellationToken,
            progress: ProgressSink,
        ) -> Result<AnalysisArtifact, AnalysisError> {
            DecodeAnalyzer.run(ctx, cancel, progress)
        }
    }
    assert!(matches!(
        runtime.execute(
            &BadProvider,
            ctx,
            CancellationToken::default(),
            ProgressSink::default()
        ),
        Err(AnalysisError::Invalid(_))
    ));
    assert_eq!(runtime.cached_entries(), 0);
}

fn waveform_fixture() -> Arc<AnalysisArtifact> {
    let mut runtime = AnalysisRuntime::new(100000);
    let mut ctx = context("stereo-48000.wav");
    let decode = execute(&mut runtime, &DecodeAnalyzer, ctx.clone()).artifact;
    ctx.dependencies.push(decode);
    execute(&mut runtime, &WaveformAnalyzer, ctx).artifact
}
fn viewport(start: f64, end: f64, pixels: u32) -> vocal_dsp::WaveformViewport {
    vocal_dsp::WaveformViewport::new(
        vocal_domain::time::Seconds::new(start).unwrap(),
        vocal_domain::time::Seconds::new(end).unwrap(),
        pixels,
    )
    .unwrap()
}

#[test]
fn viewport_chooses_resolution_and_returns_bounded_per_channel_blocks() {
    let artifact = waveform_fixture();
    for (pixels, width) in [(1, 1024), (2, 512), (4, 256), (4096, 256)] {
        let slice = vocal_dsp::query_waveform(&artifact, viewport(0.0, 0.02, pixels)).unwrap();
        assert_eq!(slice.frames_per_block, width);
        assert_eq!(slice.artifact_hash, artifact.artifact_hash());
        assert_eq!(slice.channels.len(), 2);
        assert!(slice
            .channels
            .iter()
            .all(|channel| channel.len() <= pixels as usize + 1));
    }
    for pixels in 1..=16 {
        for start_frame in [0, 1, 255, 256, 511, 900] {
            let slice = vocal_dsp::query_waveform(
                &artifact,
                viewport(start_frame as f64 / 48000.0, 0.02, pixels),
            )
            .unwrap();
            assert!(slice.channels[0].len() <= pixels as usize + 1);
        }
    }
}

#[test]
fn viewport_half_open_boundaries_and_tail_keep_real_aggregate_spans() {
    let artifact = waveform_fixture();
    let exact =
        vocal_dsp::query_waveform(&artifact, viewport(256.0 / 48000.0, 512.0 / 48000.0, 4096))
            .unwrap();
    assert_eq!(exact.channels[0].len(), 1);
    assert_eq!(
        (
            exact.channels[0][0].start.get(),
            exact.channels[0][0].end.get()
        ),
        (256, 512)
    );
    let tail = vocal_dsp::query_waveform(&artifact, viewport(900.0 / 48000.0, 10.0, 4096)).unwrap();
    assert_eq!(tail.channels[0].len(), 1);
    assert_eq!(
        (
            tail.channels[0][0].start.get(),
            tail.channels[0][0].end.get()
        ),
        (768, 960)
    );
    let ArtifactPayload::Waveform(w) = artifact.payload() else {
        panic!("waveform")
    };
    assert_eq!(tail.channels[0][0].rms, w.levels[0].channels[0][3].rms);
}

#[test]
fn viewport_clamps_preroll_and_returns_empty_for_no_intersection() {
    let artifact = waveform_fixture();
    for (start, end) in [(-2.0, -1.0), (1.0, 2.0)] {
        let slice = vocal_dsp::query_waveform(&artifact, viewport(start, end, 100)).unwrap();
        assert!(slice.channels.iter().all(Vec::is_empty));
    }
    let slice = vocal_dsp::query_waveform(&artifact, viewport(-1.0, 0.001, 100)).unwrap();
    assert_eq!(slice.channels[0][0].start.get(), 0);
}

#[test]
fn invalid_viewports_and_pcm_input_are_rejected() {
    use vocal_domain::time::Seconds;
    for (start, end, pixels) in [
        (0.0, 0.0, 10),
        (1.0, 0.0, 10),
        (0.0, 1.0, 0),
        (0.0, 1.0, 4097),
    ] {
        assert!(vocal_dsp::WaveformViewport::new(
            Seconds::new(start).unwrap(),
            Seconds::new(end).unwrap(),
            pixels
        )
        .is_err());
    }
    let decode = execute(
        &mut AnalysisRuntime::new(0),
        &DecodeAnalyzer,
        context("mono-48000.wav"),
    )
    .artifact;
    assert!(vocal_dsp::query_waveform(&decode, viewport(0.0, 0.01, 100)).is_err());
}
