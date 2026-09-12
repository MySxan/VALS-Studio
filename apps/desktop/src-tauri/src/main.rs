#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use tauri::State;
use vocal_app::{AppService, JobStatus, PlaybackDto, WaveformDto, WorkspaceDto};
mod playback;
#[cfg(test)]
mod tests;
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, vocal_app::AppError> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| "应用任务失败".to_string())?
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn current_project(service: State<'_, AppService>) -> Result<WorkspaceDto, String> {
    let s = service.inner().clone();
    blocking(move || s.current_project()).await
}
#[tauri::command]
async fn new_project(
    service: State<'_, AppService>,
    name: String,
    generation: u64,
    discard: bool,
) -> Result<WorkspaceDto, String> {
    let s = service.inner().clone();
    blocking(move || s.new_project(name, generation, discard)).await
}
#[tauri::command]
async fn open_project(
    service: State<'_, AppService>,
    path: String,
    generation: u64,
    discard: bool,
) -> Result<WorkspaceDto, String> {
    let s = service.inner().clone();
    blocking(move || s.open_project(path.into(), generation, discard)).await
}
#[tauri::command]
async fn close_project(
    service: State<'_, AppService>,
    generation: u64,
    discard: bool,
) -> Result<WorkspaceDto, String> {
    let s = service.inner().clone();
    blocking(move || s.close_project(generation, discard)).await
}
#[tauri::command]
async fn save_project(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    path: Option<String>,
) -> Result<WorkspaceDto, String> {
    let s = service.inner().clone();
    blocking(move || s.save_project(&project_id, generation, path.map(Into::into))).await
}
#[tauri::command]
async fn start_import(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    path: String,
) -> Result<String, String> {
    let s = service.inner().clone();
    blocking(move || s.start_import(&project_id, generation, path.into())).await
}
#[tauri::command]
async fn analyze_track(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
) -> Result<String, String> {
    let s = service.inner().clone();
    blocking(move || s.analyze_track(&project_id, generation, &track_id)).await
}
#[tauri::command]
async fn relink_track(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
    path: String,
) -> Result<String, String> {
    let s = service.inner().clone();
    blocking(move || s.relink_track(&project_id, generation, &track_id, path.into())).await
}
#[tauri::command]
async fn job_status(service: State<'_, AppService>, id: String) -> Result<JobStatus, String> {
    let s = service.inner().clone();
    blocking(move || s.job_status(&id)).await
}
#[tauri::command]
async fn cancel_job(service: State<'_, AppService>, id: String) -> Result<(), String> {
    let s = service.inner().clone();
    blocking(move || s.cancel_job(&id)).await
}
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn waveform_slice(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
    start: f64,
    end: f64,
    width: u32,
) -> Result<WaveformDto, String> {
    let s = service.inner().clone();
    blocking(move || s.waveform_slice(&project_id, generation, &track_id, start, end, width)).await
}
#[tauri::command]
async fn load_playback(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
    position: f64,
) -> Result<PlaybackDto, String> {
    let s = service.inner().clone();
    blocking(move || s.load_playback(&project_id, generation, &track_id, position)).await
}
#[tauri::command]
async fn playback_play(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
) -> Result<PlaybackDto, String> {
    let s = service.inner().clone();
    blocking(move || s.play(&project_id, generation, &track_id)).await
}
#[tauri::command]
async fn playback_pause(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
) -> Result<PlaybackDto, String> {
    let s = service.inner().clone();
    blocking(move || s.pause(&project_id, generation, &track_id)).await
}
#[tauri::command]
async fn playback_seek(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
    position: f64,
) -> Result<PlaybackDto, String> {
    let s = service.inner().clone();
    blocking(move || s.seek(&project_id, generation, &track_id, position)).await
}
#[tauri::command]
async fn playback_stop(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
) -> Result<PlaybackDto, String> {
    let s = service.inner().clone();
    blocking(move || s.stop(&project_id, generation, &track_id)).await
}
#[tauri::command]
async fn playback_status(
    service: State<'_, AppService>,
    project_id: String,
    generation: u64,
    track_id: String,
) -> Result<PlaybackDto, String> {
    let s = service.inner().clone();
    blocking(move || s.playback_status(&project_id, generation, &track_id)).await
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppService::with_playback(playback::CpalPlayback::default()))
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
            waveform_slice,
            load_playback,
            playback_play,
            playback_pause,
            playback_seek,
            playback_stop,
            playback_status
        ])
        .run(tauri::generate_context!())
        .expect("failed to run VALS Studio");
}
