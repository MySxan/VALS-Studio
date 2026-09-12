//! Read-only verification of files produced by the documented native UI exercise.
//! This is an example executable, never a production IPC command.
use std::{path::PathBuf, time::{Duration, Instant}};
use vocal_app::{AppService, JobPhase};
use vocal_domain::audio::AudioUri;
use vocal_project::ProjectStore;

fn main() {
    let root = PathBuf::from(std::env::args_os().nth(1).expect("usage: verify_native_acceptance <run-directory>"))
        .canonicalize().expect("acceptance directory");
    let original_path = root.join("Untitled.vocalproj");
    let relinked_path = root.join("Relinked.vocalproj");
    let original_bytes = std::fs::read(&original_path).expect("initial UI Save output");
    let relinked_bytes = std::fs::read(&relinked_path).expect("UI Save As output");
    let original = ProjectStore::default().load(&original_path).expect("load original project");
    let relinked = ProjectStore::default().load(&relinked_path).expect("load relinked project");
    assert_eq!(original.sources().len(), 1, "one fixture source expected");
    assert_eq!(original.tracks().len(), 1, "one fixture track expected");
    assert_eq!(relinked.sources().len(), 1);
    let AudioUri::Linked { relative_path, .. } = original.sources()[0].uri();
    assert_eq!(relative_path.as_deref(), Some("voice.wav"));
    let AudioUri::Linked { relative_path, .. } = relinked.sources()[0].uri();
    assert_eq!(relative_path.as_deref(), Some("relocated.wav"));
    let mut expected = original.clone();
    expected.set_source_uri(original.sources()[0].id(), relinked.sources()[0].uri().clone()).unwrap();
    assert_eq!(expected, relinked, "Relink/Save As may change location only");
    assert!(!root.join("voice.wav").exists(), "the original fixture must have been moved");
    assert!(root.join("relocated.wav").is_file());

    let service = AppService::default();
    let opened = service.open_project(relinked_path.clone(), 0, false).expect("fresh application Open");
    let project = opened.project.unwrap();
    let track_id = &project.tracks[0].id;
    let job = service.analyze_track(&project.id, opened.generation, track_id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let status = service.job_status(&job).unwrap();
        if status.phase.is_terminal() {
            assert_eq!(status.phase, JobPhase::Succeeded, "{:?}", status.error);
            break;
        }
        assert!(Instant::now() < deadline, "analysis timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    let ready = service.current_project().unwrap().project.unwrap();
    assert!(!ready.dirty);
    let wave = service.waveform_slice(&ready.id, opened.generation, track_id, 0.0,
        ready.tracks[0].duration, 2).unwrap();
    assert!(!wave.channels.is_empty());
    assert_eq!(std::fs::read(&original_path).unwrap(), original_bytes);
    assert_eq!(std::fs::read(&relinked_path).unwrap(), relinked_bytes);
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "kind": "filesystem-and-application-verification",
        "nativeUiVerifiedByThisProgram": false,
        "projectId": ready.id, "trackId": track_id,
        "sourceId": ready.tracks[0].source_id,
        "analysis": ready.tracks[0].analysis,
        "artifactHash": wave.artifact_hash,
        "projectFilesUnchanged": true
    })).unwrap());
}
