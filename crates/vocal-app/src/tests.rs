use super::*;
use std::time::{Duration, Instant};
use vocal_domain::signal::Pcm;

#[derive(Clone, Default)]
struct TestPlayback(Arc<Mutex<TestPlaybackState>>);

struct TestPlaybackState {
    phase: PlaybackPhase,
    position: u64,
    total: u64,
    stops: usize,
}

impl Default for TestPlaybackState {
    fn default() -> Self {
        Self {
            phase: PlaybackPhase::Stopped,
            position: 0,
            total: 0,
            stops: 0,
        }
    }
}

impl PlaybackEngine for TestPlayback {
    fn load(&mut self, pcm: Pcm) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        state.phase = PlaybackPhase::Stopped;
        state.position = 0;
        state.total = pcm.metadata().frames().get() as u64;
        Ok(())
    }

    fn play(&mut self) -> Result<(), String> {
        self.0.lock().unwrap().phase = PlaybackPhase::Playing;
        Ok(())
    }

    fn pause(&mut self) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        state.phase = if state.position == 0 {
            PlaybackPhase::Stopped
        } else {
            PlaybackPhase::Paused
        };
        Ok(())
    }

    fn seek(&mut self, frame: u64) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        state.position = frame.min(state.total);
        if state.phase != PlaybackPhase::Playing {
            state.phase = if state.position == 0 {
                PlaybackPhase::Stopped
            } else if state.position == state.total {
                PlaybackPhase::Ended
            } else {
                PlaybackPhase::Paused
            };
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        state.phase = PlaybackPhase::Stopped;
        state.position = 0;
        state.stops += 1;
        Ok(())
    }

    fn status(&self) -> Result<PlaybackEngineStatus, String> {
        let state = self.0.lock().unwrap();
        Ok(PlaybackEngineStatus {
            phase: state.phase,
            position_frames: state.position,
            total_frames: state.total,
        })
    }
}

#[test]
fn application_open_migrates_in_memory_and_explicit_save_writes_v2() {
    use std::io::Write;
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("v1.vocalproj");
    let id = EntityId::new().to_string();
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    archive.start_file("manifest.json", options).unwrap();
    archive.write_all(serde_json::to_string(&serde_json::json!({"format":"vocal-project","schemaVersion":1,"createdWith":"test","projectId":id})).unwrap().as_bytes()).unwrap();
    archive.start_file("project.json", options).unwrap();
    archive
        .write_all(
            serde_json::to_string(&serde_json::json!({"id":id,"name":"Legacy"}))
                .unwrap()
                .as_bytes(),
        )
        .unwrap();
    archive.finish().unwrap();
    let original = std::fs::read(&path).unwrap();
    let app = AppService::default();
    let loaded = app.open_project(path.clone(), 0, false).unwrap();
    assert_eq!(loaded.project.as_ref().unwrap().id, id);
    assert!(!loaded.project.as_ref().unwrap().dirty);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    app.save_project(&id, loaded.generation, None).unwrap();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_reader(archive.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 2);
}
pub(super) fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/stereo-48000.wav")
}
fn wait(s: &AppService, id: &str) -> JobStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let j = s.job_status(id).unwrap();
        if j.phase.is_terminal() {
            return j;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn import(s: &AppService, path: PathBuf) -> WorkspaceDto {
    let state = s.current_project().unwrap();
    let job = s
        .start_import(&state.project.unwrap().id, state.generation, path)
        .unwrap();
    assert_eq!(wait(s, &job).phase, JobPhase::Succeeded);
    s.current_project().unwrap()
}

#[test]
fn playback_is_identity_scoped_seekable_and_stopped_by_source_or_project_transitions() {
    let backend = TestPlayback::default();
    let observed = backend.clone();
    let service = AppService::with_playback(backend);
    service.new_project("playback".into(), 0, false).unwrap();
    let imported = import(&service, fixture());
    let project = imported.project.unwrap();
    let track = &project.tracks[0];

    let loaded = service
        .load_playback(&project.id, imported.generation, &track.id, 0.005)
        .unwrap();
    assert_eq!(loaded.phase, PlaybackPhase::Paused);
    assert!((loaded.position - 0.005).abs() < 1.0 / track.sample_rate as f64);
    assert_eq!(
        service
            .play(&project.id, imported.generation, &track.id)
            .unwrap()
            .phase,
        PlaybackPhase::Playing
    );
    assert_eq!(
        service
            .seek(&project.id, imported.generation, &track.id, track.duration)
            .unwrap()
            .phase,
        PlaybackPhase::Playing
    );
    assert!(matches!(
        service.seek(
            &project.id,
            imported.generation,
            &track.id,
            track.duration + 1.0
        ),
        Err(AppError::Playback(_))
    ));
    assert_eq!(
        service
            .stop(&project.id, imported.generation, &track.id)
            .unwrap()
            .position,
        0.0
    );

    service
        .load_playback(&project.id, imported.generation, &track.id, 0.0)
        .unwrap();
    let job = service
        .analyze_track(&project.id, imported.generation, &track.id)
        .unwrap();
    assert!(observed.0.lock().unwrap().stops >= 2);
    assert!(matches!(
        service.playback_status(&project.id, imported.generation, &track.id),
        Err(AppError::NotReady)
    ));
    assert_eq!(wait(&service, &job).phase, JobPhase::Succeeded);

    service
        .load_playback(&project.id, imported.generation, &track.id, 0.0)
        .unwrap();
    let next = service
        .new_project("next".into(), imported.generation, true)
        .unwrap();
    assert!(matches!(
        service.playback_status(&project.id, imported.generation, &track.id),
        Err(AppError::StaleProject)
    ));
    assert_eq!(next.generation, imported.generation + 1);
}

#[test]
fn playback_source_change_invalidates_only_derived_state() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("voice.wav");
    std::fs::copy(fixture(), &source).unwrap();
    let service = AppService::with_playback(TestPlayback::default());
    service.new_project("playback".into(), 0, false).unwrap();
    let imported = import(&service, source.clone());
    let project = imported.project.unwrap();
    service
        .save_project(
            &project.id,
            imported.generation,
            Some(temp.path().join("song.vocalproj")),
        )
        .unwrap();
    std::fs::write(source, b"changed").unwrap();

    assert!(matches!(
        service.load_playback(&project.id, imported.generation, &project.tracks[0].id, 0.0),
        Err(AppError::Playback(_))
    ));
    let current = service.current_project().unwrap().project.unwrap();
    assert_eq!(current.tracks[0].status, SourceStatus::Changed);
    assert!(current.tracks[0].analysis.is_none());
    assert!(!current.dirty);
}
#[test]
fn project_save_close_open_verify_analyze_preserves_identity() {
    let s = AppService::default();
    s.new_project("工程 α".into(), 0, false).unwrap();
    let imported = import(&s, fixture());
    let p = imported.project.unwrap();
    let track = &p.tracks[0];
    assert!(p.dirty);
    assert_eq!(track.status, SourceStatus::Ready);
    let hash = track.analysis.as_ref().unwrap().artifact_hash.clone();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("song.vocalproj");
    let saved = s
        .save_project(&p.id, imported.generation, Some(path.clone()))
        .unwrap();
    assert!(!saved.project.unwrap().dirty);
    let stored = ProjectStore::default().load(&path).unwrap();
    assert_eq!(stored.tracks()[0].id.to_string(), track.id);
    assert_eq!(stored.sources()[0].id().to_string(), track.source_id);
    let closed = s.close_project(imported.generation, false).unwrap();
    let opened = s.open_project(path, closed.generation, false).unwrap();
    let reopened = opened.project.unwrap();
    assert_eq!(reopened.id, p.id);
    assert_eq!(reopened.name, p.name);
    assert_eq!(reopened.tracks[0].status, SourceStatus::Unchecked);
    assert!(reopened.tracks[0].analysis.is_none());
    assert!(matches!(
        s.waveform_slice(&p.id, imported.generation, &track.id, 0.0, 0.02, 2),
        Err(AppError::StaleProject)
    ));
    let job = s
        .analyze_track(&p.id, opened.generation, &track.id)
        .unwrap();
    assert_eq!(wait(&s, &job).phase, JobPhase::Succeeded);
    let now = s.current_project().unwrap().project.unwrap();
    assert!(!now.dirty);
    assert_eq!(now.tracks[0].analysis.as_ref().unwrap().artifact_hash, hash);
    let wave = s
        .waveform_slice(&p.id, opened.generation, &track.id, 0.0, 0.02, 2)
        .unwrap();
    assert_eq!(wave.project_id, p.id);
    assert_eq!(wave.track_id, track.id);
    assert!(wave.channels.iter().all(|c| c.len() <= 3));
}
#[test]
fn missing_and_changed_sources_do_not_prevent_project_open() {
    for missing in [true, false] {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("audio.wav");
        std::fs::copy(fixture(), &source).unwrap();
        let s = AppService::default();
        s.new_project("test".into(), 0, false).unwrap();
        let initial = import(&s, source.clone());
        let p = initial.project.unwrap();
        let path = tmp.path().join("project.vocalproj");
        s.save_project(&p.id, initial.generation, Some(path.clone()))
            .unwrap();
        if missing {
            std::fs::remove_file(&source).unwrap();
        } else {
            std::fs::write(&source, b"changed").unwrap();
        }
        let reopened = s.open_project(path, initial.generation, false).unwrap();
        assert!(!reopened.project.as_ref().unwrap().dirty);
        let job = s
            .analyze_track(&p.id, reopened.generation, &p.tracks[0].id)
            .unwrap();
        assert_eq!(wait(&s, &job).phase, JobPhase::Failed);
        let current = s.current_project().unwrap().project.unwrap();
        assert_eq!(
            current.tracks[0].status,
            if missing {
                SourceStatus::Offline
            } else {
                SourceStatus::Changed
            }
        );
        assert!(current.tracks[0].analysis.is_none());
        assert!(!current.dirty);
        assert_eq!(current.tracks[0].source_hash, p.tracks[0].source_hash);
    }
}
#[test]
fn failed_import_does_not_attach_or_dirty_saved_project() {
    let tmp = tempfile::tempdir().unwrap();
    let s = AppService::default();
    let state = s.new_project("test".into(), 0, false).unwrap();
    let id = state.project.unwrap().id;
    s.save_project(
        &id,
        state.generation,
        Some(tmp.path().join("empty.vocalproj")),
    )
    .unwrap();
    let job = s
        .start_import(&id, state.generation, tmp.path().join("absent.wav"))
        .unwrap();
    assert_eq!(wait(&s, &job).phase, JobPhase::Failed);
    let p = s.current_project().unwrap().project.unwrap();
    assert!(p.tracks.is_empty());
    assert!(!p.dirty);
}
#[test]
fn save_failure_and_corrupt_open_preserve_current_state() {
    let tmp = tempfile::tempdir().unwrap();
    let s = AppService::default();
    let state = s.new_project("test".into(), 0, false).unwrap();
    let id = state.project.unwrap().id;
    assert!(s
        .save_project(
            &id,
            state.generation,
            Some(tmp.path().join("absent/project.vocalproj"))
        )
        .is_err());
    let before = s.current_project().unwrap();
    assert!(before.project.as_ref().unwrap().dirty);
    assert!(before.project.as_ref().unwrap().path.is_none());
    let path = tmp.path().join("bad.vocalproj");
    std::fs::write(&path, b"bad").unwrap();
    assert!(s.open_project(path, state.generation, true).is_err());
    let after = s.current_project().unwrap();
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.project.unwrap().id, id);
    assert!(matches!(
        s.close_project(state.generation, false),
        Err(AppError::UnsavedChanges)
    ));
}
#[test]
fn second_import_and_save_as_preserve_all_tracks_and_sources() {
    let tmp = tempfile::tempdir().unwrap();
    let s = AppService::default();
    s.new_project("two tracks".into(), 0, false).unwrap();
    import(&s, fixture());
    let state = import(&s, fixture());
    let p = state.project.unwrap();
    assert_eq!(p.tracks.len(), 2);
    assert_ne!(p.tracks[0].id, p.tracks[1].id);
    let first = tmp.path().join("first.vocalproj");
    let second = tmp.path().join("second.vocalproj");
    s.save_project(&p.id, state.generation, Some(first.clone()))
        .unwrap();
    s.save_project(&p.id, state.generation, Some(second.clone()))
        .unwrap();
    assert_eq!(
        ProjectStore::default().load(first).unwrap(),
        ProjectStore::default().load(second).unwrap()
    );
}
#[test]
fn generation_blocks_stale_mutations_and_viewport_validates_bounds() {
    let s = AppService::default();
    s.new_project("test".into(), 0, false).unwrap();
    let state = import(&s, fixture());
    let p = state.project.unwrap();
    for (start, end, width) in [
        (f64::NAN, 1.0, 1),
        (0.0, f64::INFINITY, 1),
        (1.0, 0.0, 1),
        (0.0, 1.0, 0),
        (0.0, 1.0, 4097),
    ] {
        assert!(matches!(
            s.waveform_slice(&p.id, state.generation, &p.tracks[0].id, start, end, width),
            Err(AppError::InvalidViewport)
        ));
    }
    s.new_project("next".into(), state.generation, true)
        .unwrap();
    assert_eq!(
        s.start_import(&p.id, state.generation, fixture()),
        Err(AppError::StaleProject)
    );
    assert!(matches!(
        s.save_project(&p.id, state.generation, None),
        Err(AppError::StaleProject)
    ));
}
#[test]
fn busy_blocks_project_mutation_until_cancelled_job_finishes() {
    let s = AppService::default();
    let state = s.new_project("test".into(), 0, false).unwrap();
    s.0.state.lock().unwrap().job = Some((
        JobStatus {
            id: "pending".into(),
            project_id: state.project.unwrap().id,
            generation: state.generation,
            track_id: None,
            phase: JobPhase::Running,
            error: None,
        },
        CancellationToken::default(),
    ));
    assert!(matches!(
        s.close_project(state.generation, true),
        Err(AppError::Busy)
    ));
    s.cancel_job("pending").unwrap();
    assert_eq!(s.job_status("pending").unwrap().phase, JobPhase::Cancelling);
    assert!(matches!(
        s.new_project("x".into(), state.generation, true),
        Err(AppError::Busy)
    ));
    assert_eq!(s.cancel_job("missing"), Err(AppError::UnknownJob));
}

#[test]
fn relink_recovers_shared_source_preserving_identity_and_cached_evidence() {
    use vocal_domain::audio::VocalTrack;
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old.wav");
    let moved = tmp.path().join("moved.wav");
    std::fs::copy(fixture(), &old).unwrap();
    let s = AppService::default();
    s.new_project("relink".into(), 0, false).unwrap();
    let initial = import(&s, old.clone());
    let p = initial.project.unwrap();
    let track = &p.tracks[0];
    // A shared source must recover for every referencing track, without duplicating sources.
    let original = {
        let mut state = s.0.state.lock().unwrap();
        let project = state.project.as_mut().unwrap();
        let mut tracks = project.domain.tracks().to_vec();
        tracks.push(VocalTrack {
            id: EntityId::new(),
            name: "shared".into(),
            source: tracks[0].source,
            channel_mode: tracks[0].channel_mode,
        });
        project.domain = VocalProject::with_audio(
            project.domain.id(),
            project.domain.name().into(),
            project.domain.sources().to_vec(),
            tracks,
        )
        .unwrap();
        project.domain.clone()
    };
    let path = tmp.path().join("relink.vocalproj");
    s.save_project(&p.id, initial.generation, Some(path.clone()))
        .unwrap();
    std::fs::rename(old, &moved).unwrap();
    let opened = s
        .open_project(path.clone(), initial.generation, false)
        .unwrap();
    let job = s
        .analyze_track(&p.id, opened.generation, &track.id)
        .unwrap();
    assert_eq!(wait(&s, &job).phase, JobPhase::Failed);
    assert_eq!(
        s.current_project().unwrap().project.unwrap().tracks[0].status,
        SourceStatus::Offline
    );
    assert_eq!(
        s.relink_track(&p.id, initial.generation, &track.id, moved.clone()),
        Err(AppError::StaleProject)
    );
    let job = s
        .relink_track(&p.id, opened.generation, &track.id, moved.clone())
        .unwrap();
    assert_eq!(wait(&s, &job).phase, JobPhase::Succeeded);
    let recovered = s.current_project().unwrap().project.unwrap();
    assert!(recovered.dirty);
    assert_eq!(recovered.id, p.id);
    for t in &recovered.tracks {
        assert_eq!(t.source_id, track.source_id);
        assert_eq!(t.status, SourceStatus::Ready);
        assert_eq!(
            t.analysis.as_ref().unwrap().artifact_hash,
            track.analysis.as_ref().unwrap().artifact_hash
        );
        assert_eq!(
            serde_json::to_value(&t.analysis).unwrap(),
            serde_json::to_value(&track.analysis).unwrap()
        );
        assert!(std::path::Path::new(&t.source_path).ends_with("moved.wav"));
        s.waveform_slice(&p.id, opened.generation, &t.id, 0.0, 0.02, 2)
            .unwrap();
    }
    s.save_project(&p.id, opened.generation, None).unwrap();
    let stored = ProjectStore::default().load(&path).unwrap();
    assert_eq!(stored.tracks(), original.tracks());
    assert_eq!(stored.sources().len(), 1);
    assert_eq!(stored.sources()[0].id(), original.sources()[0].id());
    assert_eq!(
        stored.sources()[0].content_hash(),
        original.sources()[0].content_hash()
    );
    assert_eq!(
        stored.sources()[0].metadata(),
        original.sources()[0].metadata()
    );
    assert_eq!(
        stored.sources()[0].size_bytes(),
        original.sources()[0].size_bytes()
    );
    let fresh = AppService::default();
    let reopened = fresh.open_project(path, 0, false).unwrap();
    let job = fresh
        .analyze_track(&p.id, reopened.generation, &track.id)
        .unwrap();
    assert_eq!(wait(&fresh, &job).phase, JobPhase::Succeeded);
    assert!(!fresh.current_project().unwrap().project.unwrap().dirty);
    // Re-selecting the same location does not create a semantic edit.
    let job = fresh
        .relink_track(&p.id, reopened.generation, &track.id, moved)
        .unwrap();
    assert_eq!(wait(&fresh, &job).phase, JobPhase::Succeeded);
    assert!(!fresh.current_project().unwrap().project.unwrap().dirty);
}

#[test]
fn rejected_relink_preserves_binding_dirty_and_existing_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    let s = AppService::default();
    s.new_project("test".into(), 0, false).unwrap();
    let initial = import(&s, fixture());
    let p = initial.project.unwrap();
    s.save_project(
        &p.id,
        initial.generation,
        Some(tmp.path().join("saved.vocalproj")),
    )
    .unwrap();
    let before = serde_json::to_value(s.current_project().unwrap().project).unwrap();
    let changed = tmp.path().join("changed.wav");
    let mut bytes = std::fs::read(fixture()).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1; // Valid WAV, same metadata and size, different content hash.
    std::fs::write(&changed, bytes).unwrap();
    for candidate in [changed.clone(), tmp.path().join("missing.wav")] {
        let job = s
            .relink_track(
                &p.id,
                initial.generation,
                &p.tracks[0].id,
                candidate.clone(),
            )
            .unwrap();
        let result = wait(&s, &job);
        assert_eq!(result.phase, JobPhase::Failed);
        if candidate == changed {
            assert!(result.error.unwrap().contains("relink.candidate_changed"));
        }
        assert_eq!(
            serde_json::to_value(s.current_project().unwrap().project).unwrap(),
            before
        );
    }
}

#[test]
fn relative_source_survives_directory_move_and_save_as_without_audio_copy() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old");
    let moved = tmp.path().join("moved");
    let elsewhere = tmp.path().join("elsewhere");
    std::fs::create_dir_all(old.join("audio")).unwrap();
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::copy(fixture(), old.join("audio/voice.wav")).unwrap();
    let s = AppService::default();
    s.new_project("portable".into(), 0, false).unwrap();
    let initial = import(&s, old.join("audio/voice.wav"));
    let p = initial.project.unwrap();
    let track = &p.tracks[0];
    s.save_project(&p.id, initial.generation, Some(old.join("song.vocalproj")))
        .unwrap();
    let stored = ProjectStore::default()
        .load(old.join("song.vocalproj"))
        .unwrap();
    let AudioUri::Linked { relative_path, .. } = stored.sources()[0].uri();
    assert_eq!(relative_path.as_deref(), Some("audio/voice.wav"));
    std::fs::rename(&old, &moved).unwrap();
    let fresh = AppService::default();
    let reopened = fresh
        .open_project(moved.join("song.vocalproj"), 0, false)
        .unwrap();
    let before = ProjectStore::default()
        .load(moved.join("song.vocalproj"))
        .unwrap();
    assert_eq!(before, stored); // Opening does not migrate locations or touch sources.
    let job = fresh
        .analyze_track(&p.id, reopened.generation, &track.id)
        .unwrap();
    assert_eq!(wait(&fresh, &job).phase, JobPhase::Succeeded);
    let now = fresh.current_project().unwrap().project.unwrap();
    assert!(!now.dirty);
    assert_eq!(
        now.tracks[0].source_path,
        moved
            .join("audio/voice.wav")
            .canonicalize()
            .unwrap()
            .to_string_lossy()
    );
    assert_eq!(
        now.tracks[0].analysis.as_ref().unwrap().artifact_hash,
        track.analysis.as_ref().unwrap().artifact_hash
    );
    let destination = elsewhere.join("copy.vocalproj");
    fresh
        .save_project(&p.id, reopened.generation, Some(destination.clone()))
        .unwrap();
    let copied = ProjectStore::default().load(&destination).unwrap();
    assert_eq!(copied.tracks(), stored.tracks());
    let AudioUri::Linked {
        relative_path,
        absolute_fallback,
    } = copied.sources()[0].uri();
    assert!(relative_path.is_none()); // Domain forbids ../ paths; keep the absolute location.
    assert_eq!(
        PathBuf::from(absolute_fallback),
        moved.join("audio/voice.wav").canonicalize().unwrap()
    );
    assert_eq!(std::fs::read_dir(&elsewhere).unwrap().count(), 1); // No audio copy.
    assert_eq!(
        ProjectStore::default()
            .load(moved.join("song.vocalproj"))
            .unwrap(),
        stored
    );
    let final_app = AppService::default();
    let final_open = final_app.open_project(destination, 0, false).unwrap();
    let job = final_app
        .analyze_track(&p.id, final_open.generation, &track.id)
        .unwrap();
    assert_eq!(wait(&final_app, &job).phase, JobPhase::Succeeded);
    final_app
        .waveform_slice(&p.id, final_open.generation, &track.id, 0.0, 0.02, 4)
        .unwrap();
}

#[test]
fn relative_missing_uses_fallback_but_changed_candidate_is_not_bypassed() {
    for changed in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let s = AppService::default();
        s.new_project("fallback".into(), 0, false).unwrap();
        let initial = import(&s, fixture());
        let p = initial.project.unwrap();
        let mut domain =
            s.0.state
                .lock()
                .unwrap()
                .project
                .as_ref()
                .unwrap()
                .domain
                .clone();
        let audio = domain.sources()[0].clone();
        domain
            .set_source_uri(
                audio.id(),
                AudioUri::Linked {
                    absolute_fallback: paths::fallback(&audio).to_string_lossy().into(),
                    relative_path: Some("audio\\voice.wav".into()), // Foreign separator on Unix too.
                },
            )
            .unwrap();
        let path = tmp.path().join("song.vocalproj");
        ProjectStore::default().save(&path, &domain).unwrap();
        if changed {
            std::fs::create_dir(tmp.path().join("audio")).unwrap();
            let mut bytes = std::fs::read(fixture()).unwrap();
            let last = bytes.len() - 1;
            bytes[last] ^= 1;
            std::fs::write(tmp.path().join("audio/voice.wav"), bytes).unwrap();
        }
        let opened = s.open_project(path, initial.generation, true).unwrap();
        let job = s
            .analyze_track(&p.id, opened.generation, &p.tracks[0].id)
            .unwrap();
        assert_eq!(
            wait(&s, &job).phase,
            if changed {
                JobPhase::Failed
            } else {
                JobPhase::Succeeded
            }
        );
        let now = s.current_project().unwrap().project.unwrap();
        assert_eq!(
            now.tracks[0].status,
            if changed {
                SourceStatus::Changed
            } else {
                SourceStatus::Ready
            }
        );
        assert!(!now.dirty);
        assert_eq!(
            s.0.state.lock().unwrap().project.as_ref().unwrap().domain,
            domain
        );
        if !changed {
            assert_eq!(now.tracks[0].source_path, p.tracks[0].source_path);
        }
    }
}

#[test]
fn save_as_before_analysis_rebases_offline_relative_target_and_failure_is_atomic() {
    let tmp = tempfile::tempdir().unwrap();
    let old = tmp.path().join("old");
    let new = tmp.path().join("new");
    std::fs::create_dir(&old).unwrap();
    std::fs::create_dir(&new).unwrap();
    std::fs::copy(fixture(), old.join("voice.wav")).unwrap();
    let s = AppService::default();
    s.new_project("offline".into(), 0, false).unwrap();
    let initial = import(&s, old.join("voice.wav"));
    let p = initial.project.unwrap();
    s.save_project(&p.id, initial.generation, Some(old.join("song.vocalproj")))
        .unwrap();
    std::fs::remove_file(old.join("voice.wav")).unwrap();
    let opened = s
        .open_project(old.join("song.vocalproj"), initial.generation, false)
        .unwrap();
    let before = serde_json::to_value(s.current_project().unwrap().project).unwrap();
    // Existing directory with the project suffix fails at atomic persistence, after rebasing.
    let bad = new.join("blocked.vocalproj");
    std::fs::create_dir(&bad).unwrap();
    assert!(s.save_project(&p.id, opened.generation, Some(bad)).is_err());
    assert_eq!(
        serde_json::to_value(s.current_project().unwrap().project).unwrap(),
        before
    );
    let copy = new.join("copy.vocalproj");
    s.save_project(&p.id, opened.generation, Some(copy.clone()))
        .unwrap();
    let stored = ProjectStore::default().load(&copy).unwrap();
    let AudioUri::Linked {
        relative_path,
        absolute_fallback,
    } = stored.sources()[0].uri();
    assert!(relative_path.is_none());
    assert_eq!(
        PathBuf::from(absolute_fallback),
        old.canonicalize().unwrap().join("voice.wav")
    );
    let opened = s.open_project(copy, opened.generation, false).unwrap();
    let job = s
        .analyze_track(&p.id, opened.generation, &p.tracks[0].id)
        .unwrap();
    assert_eq!(wait(&s, &job).phase, JobPhase::Failed);
    assert_eq!(
        s.current_project().unwrap().project.unwrap().tracks[0].status,
        SourceStatus::Offline
    );
}

#[test]
fn save_cannot_overwrite_a_relative_audio_candidate() {
    let tmp = tempfile::tempdir().unwrap();
    let s = AppService::default();
    s.new_project("guard".into(), 0, false).unwrap();
    let initial = import(&s, fixture());
    let p = initial.project.unwrap();
    let mut domain =
        s.0.state
            .lock()
            .unwrap()
            .project
            .as_ref()
            .unwrap()
            .domain
            .clone();
    let id = domain.sources()[0].id();
    domain
        .set_source_uri(
            id,
            AudioUri::Linked {
                absolute_fallback: "Z:/missing/voice.wav".into(),
                relative_path: Some("audio.vocalproj".into()),
            },
        )
        .unwrap();
    let path = tmp.path().join("song.vocalproj");
    ProjectStore::default().save(&path, &domain).unwrap();
    let audio_path = tmp.path().join("audio.vocalproj");
    std::fs::copy(fixture(), &audio_path).unwrap();
    let opened = s.open_project(path, initial.generation, true).unwrap();
    assert!(matches!(
        s.save_project(&p.id, opened.generation, Some(audio_path.clone())),
        Err(AppError::InvalidPath)
    ));
    assert_eq!(
        std::fs::read(&audio_path).unwrap(),
        std::fs::read(fixture()).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn relative_symlink_cannot_escape_project_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let s = AppService::default();
    s.new_project("symlink".into(), 0, false).unwrap();
    let initial = import(&s, fixture());
    let p = initial.project.unwrap();
    let mut domain =
        s.0.state
            .lock()
            .unwrap()
            .project
            .as_ref()
            .unwrap()
            .domain
            .clone();
    let source = domain.sources()[0].clone();
    domain
        .set_source_uri(
            source.id(),
            AudioUri::Linked {
                absolute_fallback: paths::fallback(&source).to_string_lossy().into(),
                relative_path: Some("escape.wav".into()),
            },
        )
        .unwrap();
    std::os::unix::fs::symlink(fixture(), tmp.path().join("escape.wav")).unwrap();
    let path = tmp.path().join("song.vocalproj");
    ProjectStore::default().save(&path, &domain).unwrap();
    let opened = s.open_project(path, initial.generation, true).unwrap();
    let job = s
        .analyze_track(&p.id, opened.generation, &p.tracks[0].id)
        .unwrap();
    let result = wait(&s, &job);
    assert_eq!(result.phase, JobPhase::Failed);
    assert!(result.error.unwrap().contains("escapes project directory"));
    assert_eq!(
        s.0.state.lock().unwrap().project.as_ref().unwrap().domain,
        domain
    );
}
