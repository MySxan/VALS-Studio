#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::State;
use vocal_app::{AppError, AppService, JobStatus, SessionDto, WaveformDto};

#[cfg(test)]
mod tests;

fn error(e: AppError) -> String {
    match e {
        AppError::Busy => "已有分析任务正在运行",
        AppError::UnknownJob => "任务不存在",
        AppError::StaleSession => "预览会话已更新",
        AppError::InvalidViewport => "无效的波形视口",
        AppError::Unavailable => "分析服务不可用",
    }
    .into()
}
#[tauri::command]
fn start_import(service: State<'_, AppService>, path: String) -> Result<String, String> {
    service.start_import(path.into()).map_err(error)
}
#[tauri::command]
fn job_status(service: State<'_, AppService>, id: String) -> Result<JobStatus, String> {
    service.job_status(&id).map_err(error)
}
#[tauri::command]
fn cancel_job(service: State<'_, AppService>, id: String) -> Result<(), String> {
    service.cancel_job(&id).map_err(error)
}
#[tauri::command]
fn current_session(service: State<'_, AppService>) -> Result<Option<SessionDto>, String> {
    service.current_session().map_err(error)
}
#[tauri::command]
async fn waveform_slice(
    service: State<'_, AppService>,
    session_id: String,
    start: f64,
    end: f64,
    width: u32,
) -> Result<WaveformDto, String> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service
            .waveform_slice(&session_id, start, end, width)
            .map_err(error)
    })
    .await
    .map_err(|_| "波形查询任务失败".to_string())?
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppService::default())
        .invoke_handler(tauri::generate_handler![
            start_import,
            job_status,
            cancel_job,
            current_session,
            waveform_slice
        ])
        .run(tauri::generate_context!())
        .expect("failed to run VALS Studio");
}
