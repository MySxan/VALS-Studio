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
    let job = service.start_import(path).expect("start import");
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
    let session = service.current_session().unwrap().unwrap();
    let waveform = service
        .waveform_slice(&session.id, 0.0, session.duration, 2)
        .unwrap();
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({ "session": session, "waveform": waveform })
        )
        .unwrap()
    );
}
