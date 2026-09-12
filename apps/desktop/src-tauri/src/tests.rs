use super::*;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::test::{mock_builder, mock_context, noop_assets, MockRuntime};
fn invoke(
    w: &tauri::WebviewWindow<MockRuntime>,
    command: &str,
    body: Value,
) -> Result<Value, Value> {
    tauri::test::get_ipc_response(
        w,
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
    .map(|v| v.deserialize().unwrap())
}
fn wait(w: &tauri::WebviewWindow<MockRuntime>, id: Value) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let job = invoke(w, "job_status", json!({"id": id})).unwrap();
        if job["phase"] == "succeeded" {
            break;
        }
        assert_eq!(job["phase"], "running", "{job}");
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn desktop_project_roundtrip_uses_real_store_analysis_and_ipc() {
    let app = mock_builder()
        .manage(AppService::default())
        .invoke_handler(tauri::generate_handler![
            current_project,
            new_project,
            open_project,
            close_project,
            save_project,
            start_import,
            analyze_track,
            relink_track,
            job_status,
            cancel_job,
            waveform_slice
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let w = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let state = invoke(
        &w,
        "new_project",
        json!({"name":"IPC project", "generation":0,"discard":false}),
    )
    .unwrap();
    let id = &state["project"]["id"];
    let generation = &state["generation"];
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/audio/stereo-48000.wav");
    wait(
        &w,
        invoke(
            &w,
            "start_import",
            json!({"projectId":id,"generation":generation,"path":path}),
        )
        .unwrap(),
    );
    let state = invoke(&w, "current_project", json!({})).unwrap();
    let track = &state["project"]["tracks"][0];
    let temp = tempfile::tempdir().unwrap();
    let saved_path = temp.path().join("ipc.vocalproj");
    let saved = invoke(
        &w,
        "save_project",
        json!({"projectId":id,"generation":generation,"path":saved_path}),
    )
    .unwrap();
    assert_eq!(saved["project"]["dirty"], false);
    let closed = invoke(
        &w,
        "close_project",
        json!({"generation":generation,"discard":false}),
    )
    .unwrap();
    let reopened = invoke(
        &w,
        "open_project",
        json!({"path":saved_path,"generation":closed["generation"],"discard":false}),
    )
    .unwrap();
    assert_eq!(reopened["project"]["tracks"][0]["id"], track["id"]);
    assert_eq!(reopened["project"]["tracks"][0]["status"], "unchecked");
    wait(
        &w,
        invoke(
            &w,
            "analyze_track",
            json!({"projectId":id,"generation":reopened["generation"],"trackId":track["id"]}),
        )
        .unwrap(),
    );
    let view = json!({"projectId":id,"generation":reopened["generation"],"trackId":track["id"],"start":0.0,"end":0.02,"width":2});
    let wave = invoke(&w, "waveform_slice", view.clone()).unwrap();
    assert_eq!(wave["trackId"], track["id"]);
    assert_eq!(wave["artifactHash"], track["analysis"]["artifactHash"]);
    let candidate = temp.path().join("relocated.wav");
    std::fs::copy(&path, &candidate).unwrap();
    wait(
        &w,
        invoke(
            &w,
            "relink_track",
            json!({"projectId":id,
        "generation":reopened["generation"],"trackId":track["id"],"path":candidate}),
        )
        .unwrap(),
    );
    let linked = invoke(&w, "current_project", json!({})).unwrap();
    assert_eq!(linked["project"]["dirty"], true);
    assert_eq!(
        linked["project"]["tracks"][0]["sourceId"],
        track["sourceId"]
    );
    assert_eq!(
        linked["project"]["tracks"][0]["analysis"],
        track["analysis"]
    );
    assert!(linked["project"]["tracks"][0]["sourcePath"]
        .as_str()
        .unwrap()
        .ends_with("relocated.wav"));
    assert_eq!(invoke(&w, "waveform_slice", view.clone()).unwrap(), wave);
    assert!(invoke(
        &w,
        "relink_track",
        json!({"projectId":id,"generation":generation,
        "trackId":track["id"],"path":candidate})
    )
    .is_err());
    let mut stale = view.clone();
    stale["generation"] = generation.clone();
    assert!(invoke(&w, "waveform_slice", stale).is_err());
    let mut invalid = view;
    invalid["width"] = json!(4097);
    assert!(invoke(&w, "waveform_slice", invalid).is_err());
    assert!(invoke(&w, "start_import", json!({"path":123})).is_err());
}

#[test]
fn desktop_directory_move_and_save_as_resolve_relative_sources_through_ipc() {
    let app = mock_builder()
        .manage(AppService::default())
        .invoke_handler(tauri::generate_handler![
            current_project,
            new_project,
            open_project,
            save_project,
            start_import,
            analyze_track,
            job_status,
            waveform_slice
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let w = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let old = temp.path().join("old");
    let moved = temp.path().join("moved");
    std::fs::create_dir_all(old.join("audio")).unwrap();
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/audio/stereo-48000.wav");
    std::fs::copy(fixture, old.join("audio/voice.wav")).unwrap();
    let initial = invoke(
        &w,
        "new_project",
        json!({"name":"Portable","generation":0,"discard":false}),
    )
    .unwrap();
    let id = &initial["project"]["id"];
    wait(
        &w,
        invoke(
            &w,
            "start_import",
            json!({"projectId":id,"generation":initial["generation"],
        "path":old.join("audio/voice.wav")}),
        )
        .unwrap(),
    );
    let imported = invoke(&w, "current_project", json!({})).unwrap();
    let track = &imported["project"]["tracks"][0];
    invoke(
        &w,
        "save_project",
        json!({"projectId":id,"generation":initial["generation"],
        "path":old.join("song.vocalproj")}),
    )
    .unwrap();
    std::fs::rename(old, &moved).unwrap();
    let opened = invoke(
        &w,
        "open_project",
        json!({"path":moved.join("song.vocalproj"),
        "generation":initial["generation"],"discard":false}),
    )
    .unwrap();
    assert_eq!(opened["project"]["tracks"][0]["status"], "unchecked");
    wait(
        &w,
        invoke(
            &w,
            "analyze_track",
            json!({"projectId":id,"generation":opened["generation"],
        "trackId":track["id"]}),
        )
        .unwrap(),
    );
    let ready = invoke(&w, "current_project", json!({})).unwrap();
    assert_eq!(ready["project"]["dirty"], false);
    let expected = moved.join("audio/voice.wav").canonicalize().unwrap();
    assert_eq!(
        ready["project"]["tracks"][0]["sourcePath"],
        expected.to_string_lossy().as_ref()
    );
    let copy = temp.path().join("copy.vocalproj");
    invoke(
        &w,
        "save_project",
        json!({"projectId":id,"generation":opened["generation"],"path":copy}),
    )
    .unwrap();
    let reopened = invoke(
        &w,
        "open_project",
        json!({"path":copy,"generation":opened["generation"],"discard":false}),
    )
    .unwrap();
    wait(
        &w,
        invoke(
            &w,
            "analyze_track",
            json!({"projectId":id,"generation":reopened["generation"],"trackId":track["id"]}),
        )
        .unwrap(),
    );
    let wave = invoke(
        &w,
        "waveform_slice",
        json!({"projectId":id,"generation":reopened["generation"],
        "trackId":track["id"],"start":0.0,"end":0.02,"width":2}),
    )
    .unwrap();
    assert_eq!(wave["artifactHash"], track["analysis"]["artifactHash"]);
    assert_eq!(wave["trackId"], track["id"]);
}
