use super::*;
use std::time::{Duration, Instant};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/stereo-48000.wav")
}
fn wait(service: &AppService, id: &str) -> JobStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = service.job_status(id).unwrap();
        if status.phase.is_terminal() {
            return status;
        }
        assert!(Instant::now() < deadline, "background job timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn import(service: &AppService) -> SessionDto {
    let id = service.start_import(fixture()).unwrap();
    assert_eq!(wait(service, &id).phase, JobPhase::Succeeded);
    service.current_session().unwrap().unwrap()
}
fn pending(service: &AppService) -> String {
    let id = EntityId::new().to_string();
    service.0.state.lock().unwrap().job = Some((
        JobStatus {
            id: id.clone(),
            phase: JobPhase::Running,
            error: None,
        },
        CancellationToken::default(),
    ));
    id
}

#[test]
fn real_background_import_exports_bounded_dto_with_evidence() {
    let service = AppService::default();
    assert!(service.current_session().unwrap().is_none());
    let session = import(&service);
    assert_eq!(session.channels, 2);
    assert_eq!(session.sample_rate, 48000);
    assert_eq!(session.confidence.kind, "measurement");
    assert_eq!(session.confidence.score, None);
    assert_eq!(session.provenance.source_hash, session.source_hash);
    assert_eq!(session.provenance.dependency_hashes.len(), 1);
    assert!(!session.provenance.settings_hash.is_empty());
    let slice = service
        .waveform_slice(&session.id, 0.0, session.duration, 2)
        .unwrap();
    assert_eq!(slice.session_id, session.id);
    assert_eq!(slice.artifact_hash, session.artifact_hash);
    assert!(slice.channels.iter().all(|c| c.len() <= 3));
    let json = serde_json::to_value(&session).unwrap();
    assert_eq!(json["provenance"]["sourceHash"], session.source_hash);
    assert!(json.get("pcm").is_none());
    assert!(serde_json::to_string(&slice).unwrap().len() < 4096);
}

#[test]
fn failed_import_preserves_previous_session() {
    let service = AppService::default();
    let old = import(&service);
    let job = service
        .start_import(fixture().with_file_name("absent.wav"))
        .unwrap();
    let status = wait(&service, &job);
    assert_eq!(status.phase, JobPhase::Failed);
    assert!(status.error.is_some());
    assert_eq!(service.current_session().unwrap().unwrap().id, old.id);
}

#[test]
fn cancellation_wins_before_publication_even_after_analysis_completed() {
    let service = AppService::default();
    let old = import(&service);
    let candidate = service
        .analyze(fixture(), &CancellationToken::default())
        .unwrap();
    let id = pending(&service);
    service.cancel_job(&id).unwrap();
    assert_eq!(service.job_status(&id).unwrap().phase, JobPhase::Cancelling);
    assert_eq!(service.start_import(fixture()), Err(AppError::Busy));
    service.finish(&id, Ok(candidate));
    assert_eq!(service.job_status(&id).unwrap().phase, JobPhase::Cancelled);
    assert_eq!(service.current_session().unwrap().unwrap().id, old.id);
}

#[test]
fn pre_cancelled_analysis_exits_and_does_not_cache() {
    let service = AppService::default();
    let token = CancellationToken::default();
    token.cancel();
    assert!(service.analyze(fixture(), &token).is_err());
    assert_eq!(service.0.runtime.lock().unwrap().cached_entries(), 0);
}

#[test]
fn replacement_reuses_artifacts_and_rejects_old_session_queries() {
    let service = AppService::default();
    let old = import(&service);
    let entries = service.0.runtime.lock().unwrap().cached_entries();
    let new = import(&service);
    assert_ne!(old.id, new.id);
    assert_eq!(old.artifact_hash, new.artifact_hash);
    assert_eq!(service.0.runtime.lock().unwrap().cached_entries(), entries);
    assert!(matches!(
        service.waveform_slice(&old.id, 0.0, 1.0, 100),
        Err(AppError::StaleSession)
    ));
    for _ in 0..3 {
        service.waveform_slice(&new.id, 0.0, 0.01, 100).unwrap();
    }
    assert_eq!(service.0.runtime.lock().unwrap().cached_entries(), entries);
}

#[test]
fn invalid_ipc_viewports_are_rejected() {
    let service = AppService::default();
    let session = import(&service);
    for (start, end, width) in [
        (f64::NAN, 1.0, 1),
        (0.0, f64::INFINITY, 1),
        (1.0, 0.0, 1),
        (0.0, 1.0, 0),
        (0.0, 1.0, 4097),
    ] {
        assert!(matches!(
            service.waveform_slice(&session.id, start, end, width),
            Err(AppError::InvalidViewport)
        ));
    }
}

#[test]
fn unknown_jobs_and_stale_completion_cannot_affect_active_job() {
    let service = AppService::default();
    assert!(matches!(
        service.job_status("unknown"),
        Err(AppError::UnknownJob)
    ));
    assert_eq!(service.cancel_job("unknown"), Err(AppError::UnknownJob));
    let id = pending(&service);
    assert_eq!(service.start_import(fixture()), Err(AppError::Busy));
    service.finish("outdated", Err("must not publish".into()));
    assert_eq!(service.job_status(&id).unwrap().phase, JobPhase::Running);
    service.finish(&id, Err("provider failure".into()));
    service.cancel_job(&id).unwrap();
    assert_eq!(service.job_status(&id).unwrap().phase, JobPhase::Failed);
}
