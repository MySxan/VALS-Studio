//! Project-backed orchestration. Artifacts are derived, never semantic edits.
mod dto;
mod jobs;
mod paths;
#[cfg(test)]
mod tests;
pub use dto::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use vocal_analysis_api::{AnalysisArtifact, CancellationToken};
use vocal_analysis_runtime::AnalysisRuntime;
use vocal_domain::{audio::AudioUri, identity::EntityId, project::VocalProject, time::Seconds};
use vocal_dsp::{query_waveform, WaveformViewport};
use vocal_project::ProjectStore;

#[derive(Clone)]
pub struct AppService(Arc<Inner>);
struct Inner {
    state: Mutex<State>,
    runtime: Mutex<AnalysisRuntime>,
}
#[derive(Default)]
struct State {
    generation: u64,
    project: Option<OpenProject>,
    job: Option<(JobStatus, CancellationToken)>,
}
struct OpenProject {
    domain: VocalProject,
    path: Option<PathBuf>,
    dirty: bool,
    derived: HashMap<EntityId, DerivedTrack>,
}
#[derive(Default)]
struct DerivedTrack {
    status: SourceStatus,
    error: Option<String>,
    artifact: Option<Arc<AnalysisArtifact>>,
    resolved_path: Option<PathBuf>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Busy,
    UnknownJob,
    StaleProject,
    UnknownTrack,
    NotReady,
    UnsavedChanges,
    InvalidPath,
    InvalidViewport,
    Unavailable,
    Storage(String),
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Busy => "已有后台任务正在运行",
            Self::UnknownJob => "任务不存在",
            Self::StaleProject => "工程已切换，请刷新状态",
            Self::UnknownTrack => "轨道不存在",
            Self::NotReady => "波形尚未就绪",
            Self::UnsavedChanges => "工程有未保存的修改",
            Self::InvalidPath => "请选择 .vocalproj 路径，且不可覆盖音频源",
            Self::InvalidViewport => "无效的波形视口",
            Self::Unavailable => "应用服务不可用",
            Self::Storage(message) => message,
        })
    }
}
impl std::error::Error for AppError {}
impl Default for AppService {
    fn default() -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(State::default()),
            runtime: Mutex::new(AnalysisRuntime::new(128 * 1024 * 1024)),
        }))
    }
}
impl State {
    fn check(&self, generation: u64) -> Result<(), AppError> {
        if self.generation != generation {
            Err(AppError::StaleProject)
        } else {
            Ok(())
        }
    }
    fn idle(&self) -> Result<(), AppError> {
        if self
            .job
            .as_ref()
            .is_some_and(|(j, _)| !j.phase.is_terminal())
        {
            Err(AppError::Busy)
        } else {
            Ok(())
        }
    }
    fn replaceable(&self, generation: u64, discard: bool) -> Result<(), AppError> {
        self.check(generation)?;
        self.idle()?;
        if !discard && self.project.as_ref().is_some_and(|p| p.dirty) {
            return Err(AppError::UnsavedChanges);
        }
        Ok(())
    }
    fn project(&self, id: &str, generation: u64) -> Result<&OpenProject, AppError> {
        self.check(generation)?;
        self.project
            .as_ref()
            .filter(|p| p.domain.id().to_string() == id)
            .ok_or(AppError::StaleProject)
    }
    fn snapshot(&self) -> WorkspaceDto {
        WorkspaceDto {
            generation: self.generation,
            project: self.project.as_ref().map(ProjectDto::from_project),
            job: self.job.as_ref().map(|(j, _)| j.clone()),
        }
    }
}
impl AppService {
    pub fn current_project(&self) -> Result<WorkspaceDto, AppError> {
        Ok(self
            .0
            .state
            .lock()
            .map_err(|_| AppError::Unavailable)?
            .snapshot())
    }
    pub fn new_project(
        &self,
        name: String,
        generation: u64,
        discard: bool,
    ) -> Result<WorkspaceDto, AppError> {
        let mut state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        state.replaceable(generation, discard)?;
        state.generation += 1;
        state.project = Some(OpenProject {
            domain: VocalProject::new(name),
            path: None,
            dirty: true,
            derived: HashMap::new(),
        });
        state.job = None;
        Ok(state.snapshot())
    }
    /// Metadata-only load: no audio verification or implicit migration writeback.
    pub fn open_project(
        &self,
        path: PathBuf,
        generation: u64,
        discard: bool,
    ) -> Result<WorkspaceDto, AppError> {
        let mut state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        state.replaceable(generation, discard)?;
        let path = path
            .canonicalize()
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let domain = ProjectStore::default()
            .load(&path)
            .map_err(|e| AppError::Storage(e.to_string()))?;
        state.generation += 1;
        state.project = Some(OpenProject {
            domain,
            path: Some(path),
            dirty: false,
            derived: HashMap::new(),
        });
        state.job = None;
        Ok(state.snapshot())
    }
    pub fn close_project(&self, generation: u64, discard: bool) -> Result<WorkspaceDto, AppError> {
        let mut state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        state.replaceable(generation, discard)?;
        state.generation += 1;
        state.project = None;
        state.job = None;
        Ok(state.snapshot())
    }
    /// Serializes saves and mutations. Only a successful atomic commit clears dirty.
    pub fn save_project(
        &self,
        project_id: &str,
        generation: u64,
        path: Option<PathBuf>,
    ) -> Result<WorkspaceDto, AppError> {
        let mut state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        let project = state.project(project_id, generation)?;
        state.idle()?;
        let path = path
            .or_else(|| project.path.clone())
            .ok_or(AppError::InvalidPath)?;
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("vocalproj"))
        {
            return Err(AppError::InvalidPath);
        }
        let path = paths::absolute_project_path(&path)?;
        if let Ok(target) = path.canonicalize() {
            for source in project.domain.sources() {
                let candidates = [
                    Some(paths::fallback(source)),
                    paths::relative_candidate(source, project.path.as_deref()),
                ];
                if candidates
                    .into_iter()
                    .flatten()
                    .any(|p| p.canonicalize().ok().as_ref() == Some(&target))
                {
                    return Err(AppError::InvalidPath);
                }
            }
        }
        let domain = paths::saved_domain(project, &path)?;
        ProjectStore::default()
            .save(&path, &domain)
            .map_err(|e| AppError::Storage(e.to_string()))?;
        let project = state.project.as_mut().ok_or(AppError::StaleProject)?;
        project.domain = domain;
        project.path = Some(path);
        project.dirty = false;
        Ok(state.snapshot())
    }
    pub fn job_status(&self, id: &str) -> Result<JobStatus, AppError> {
        self.0
            .state
            .lock()
            .map_err(|_| AppError::Unavailable)?
            .job
            .as_ref()
            .filter(|(j, _)| j.id == id)
            .map(|(j, _)| j.clone())
            .ok_or(AppError::UnknownJob)
    }
    pub fn cancel_job(&self, id: &str) -> Result<(), AppError> {
        let mut state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        let (job, token) = state
            .job
            .as_mut()
            .filter(|(j, _)| j.id == id)
            .ok_or(AppError::UnknownJob)?;
        if !job.phase.is_terminal() {
            token.cancel();
            job.phase = JobPhase::Cancelling;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)] // Mirrors the bounded IPC window query.
    pub fn waveform_slice(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
        start: f64,
        end: f64,
        width: u32,
    ) -> Result<WaveformDto, AppError> {
        let artifact = {
            let state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
            let project = state.project(project_id, generation)?;
            let id = track_id
                .parse::<EntityId>()
                .map_err(|_| AppError::UnknownTrack)?;
            if !project.domain.tracks().iter().any(|t| t.id == id) {
                return Err(AppError::UnknownTrack);
            }
            project
                .derived
                .get(&id)
                .and_then(|d| d.artifact.clone())
                .ok_or(AppError::NotReady)?
        };
        let start = Seconds::new(start).map_err(|_| AppError::InvalidViewport)?;
        let end = Seconds::new(end).map_err(|_| AppError::InvalidViewport)?;
        let view =
            WaveformViewport::new(start, end, width).map_err(|_| AppError::InvalidViewport)?;
        let slice = query_waveform(&artifact, view).map_err(|_| AppError::InvalidViewport)?;
        Ok(WaveformDto {
            project_id: project_id.into(),
            generation,
            track_id: track_id.into(),
            artifact_hash: slice.artifact_hash.to_string(),
            frames_per_block: slice.frames_per_block,
            channels: slice
                .channels
                .into_iter()
                .map(|c| {
                    c.into_iter()
                        .map(|p| PointDto {
                            start: p.start.get(),
                            end: p.end.get(),
                            min: p.min,
                            max: p.max,
                            rms: p.rms,
                        })
                        .collect()
                })
                .collect(),
        })
    }
}
