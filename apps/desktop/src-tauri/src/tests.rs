use super::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};

fn invoke(
    window: &tauri::WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
) -> Result<Value, Value> {
    tauri::test::get_ipc_response(
        window,
        tauri::webview::InvokeRequest {
            cmd: command.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "http://tauri.localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|value| value.deserialize().unwrap())
}

#[test]
fn commands_import_real_wav_and_return_bounded_camel_case_dtos() {
    let app = mock_builder()
        .manage(AppService::default())
        .invoke_handler(tauri::generate_handler![
            start_import,
            job_status,
            cancel_job,
            current_session,
            waveform_slice
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    assert_eq!(
        invoke(&window, "current_session", json!({})).unwrap(),
        Value::Null
    );
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/audio/stereo-48000.wav");
    let id = invoke(&window, "start_import", json!({ "path": path })).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let status = invoke(&window, "job_status", json!({ "id": id })).unwrap();
        if status["phase"] == "succeeded" {
            break;
        }
        assert_eq!(status["phase"], "running", "{status}");
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let session = invoke(&window, "current_session", json!({})).unwrap();
    assert_eq!(session["sampleRate"], 48000);
    assert_eq!(session["confidence"]["score"], Value::Null);
    let view = json!({ "sessionId": session["id"], "start": 0.0, "end": 0.02, "width": 2 });
    let waveform = invoke(&window, "waveform_slice", view.clone()).unwrap();
    assert_eq!(waveform["artifactHash"], session["artifactHash"]);
    assert_eq!(waveform["sessionId"], session["id"]);
    assert!(waveform["channels"][0].as_array().unwrap().len() <= 3);
    let mut invalid = view;
    invalid["width"] = json!(4097);
    assert!(invoke(&window, "waveform_slice", invalid).is_err());
    assert!(invoke(&window, "cancel_job", json!({"id": "missing"})).is_err());
    assert!(invoke(&window, "start_import", json!({"path": 123})).is_err());
}
