use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use vocal_app::{AppService, JobPhase};

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio/stereo-48000.wav")
        });
    let service = AppService::default();
    let workspace = service
        .new_project("Example Project".into(), 0, false)
        .unwrap();
    let project_id = workspace.project.unwrap().id;
    let job = service
        .start_import(&project_id, workspace.generation, path)
        .expect("start import");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let status = service.job_status(&job).expect("job status");
        if status.phase.is_terminal() {
            assert_eq!(status.phase, JobPhase::Succeeded, "{:?}", status.error);
            break;
        }
        assert!(Instant::now() < deadline, "import timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    let workspace = service.current_project().unwrap();
    let track = &workspace.project.as_ref().unwrap().tracks[0];
    let waveform = service
        .waveform_slice(
            &project_id,
            workspace.generation,
            &track.id,
            0.0,
            track.duration,
            2,
        )
        .unwrap();
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({ "workspace": workspace, "waveform": waveform })
        )
        .unwrap()
    );
}
