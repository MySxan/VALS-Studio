//! Application orchestration. The domain and analyzers have no dependency on UI DTOs.
mod dto;
#[cfg(test)]
mod tests;
pub use dto::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use vocal_analysis_api::{AnalysisArtifact, AnalysisContext, CancellationToken, ProgressSink};
use vocal_analysis_runtime::AnalysisRuntime;
use vocal_audio::{DecodeAnalyzer, WavImporter};
use vocal_domain::{audio::AudioSource, identity::EntityId, time::Seconds};
use vocal_dsp::{query_waveform, WaveformAnalyzer, WaveformViewport};

#[derive(Clone)]
pub struct AppService(Arc<Inner>);
struct Inner {
    state: Mutex<State>,
    runtime: Mutex<AnalysisRuntime>,
}
#[derive(Default)]
struct State {
    job: Option<(JobStatus, CancellationToken)>,
    session: Option<Arc<Session>>,
}
struct Session {
    id: String,
    source: AudioSource,
    waveform: Arc<AnalysisArtifact>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppError {
    Busy,
    UnknownJob,
    StaleSession,
    InvalidViewport,
    Unavailable,
}
impl Default for AppService {
    fn default() -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(State::default()),
            runtime: Mutex::new(AnalysisRuntime::new(128 * 1024 * 1024)),
        }))
    }
}
impl AppService {
    /// Starts a single background import. No project file or existing session is modified.
    pub fn start_import(&self, path: PathBuf) -> Result<String, AppError> {
        let mut state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        if state
            .job
            .as_ref()
            .is_some_and(|(j, _)| !j.phase.is_terminal())
        {
            return Err(AppError::Busy);
        }
        let id = EntityId::new().to_string();
        let token = CancellationToken::default();
        state.job = Some((
            JobStatus {
                id: id.clone(),
                phase: JobPhase::Running,
                error: None,
            },
            token.clone(),
        ));
        let service = self.clone();
        let worker_id = id.clone();
        // Hold the state lock until spawn succeeds; a failed spawn cannot strand Running.
        if std::thread::Builder::new()
            .name("vocal-import".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    service.analyze(path, &token)
                }));
                let result = result.unwrap_or_else(|_| Err("analysis.worker_panicked".into()));
                service.finish(&worker_id, result);
            })
            .is_err()
        {
            state.job = None;
            return Err(AppError::Unavailable);
        }
        Ok(id)
    }
    fn analyze(&self, path: PathBuf, token: &CancellationToken) -> Result<Session, String> {
        let source = WavImporter::import_linked(path, token).map_err(|e| e.to_string())?;
        let mut runtime = self
            .0
            .runtime
            .lock()
            .map_err(|_| "analysis.runtime_unavailable")?;
        let mut context = AnalysisContext::new(source.clone());
        let decode = runtime
            .execute(
                &DecodeAnalyzer,
                context.clone(),
                token.clone(),
                ProgressSink::default(),
            )
            .map_err(|e| e.to_string())?;
        context.dependencies.push(decode.artifact);
        let wave = runtime
            .execute(
                &WaveformAnalyzer,
                context,
                token.clone(),
                ProgressSink::default(),
            )
            .map_err(|e| e.to_string())?;
        Ok(Session {
            id: EntityId::new().to_string(),
            source,
            waveform: wave.artifact,
        })
    }
    fn finish(&self, id: &str, result: Result<Session, String>) {
        let Ok(mut state) = self.0.state.lock() else {
            return;
        };
        let Some((job, token)) = state.job.as_mut() else {
            return;
        };
        if job.id != id {
            return;
        }
        // Cancellation and publication share this lock: cancellation winning prevents publication.
        if token.is_cancelled() {
            job.phase = JobPhase::Cancelled;
            return;
        }
        match result {
            Ok(session) => {
                job.phase = JobPhase::Succeeded;
                state.session = Some(Arc::new(session));
            }
            Err(error) => {
                job.phase = JobPhase::Failed;
                job.error = Some(error);
            }
        }
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
    pub fn current_session(&self) -> Result<Option<SessionDto>, AppError> {
        let state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        Ok(state.session.as_ref().map(|s| SessionDto::from_session(s)))
    }
    pub fn waveform_slice(
        &self,
        session_id: &str,
        start: f64,
        end: f64,
        width: u32,
    ) -> Result<WaveformDto, AppError> {
        let session = self
            .0
            .state
            .lock()
            .map_err(|_| AppError::Unavailable)?
            .session
            .clone()
            .filter(|s| s.id == session_id)
            .ok_or(AppError::StaleSession)?;
        let start = Seconds::new(start).map_err(|_| AppError::InvalidViewport)?;
        let end = Seconds::new(end).map_err(|_| AppError::InvalidViewport)?;
        let viewport =
            WaveformViewport::new(start, end, width).map_err(|_| AppError::InvalidViewport)?;
        let slice =
            query_waveform(&session.waveform, viewport).map_err(|_| AppError::InvalidViewport)?;
        Ok(WaveformDto {
            session_id: session.id.clone(),
            artifact_hash: slice.artifact_hash.to_string(),
            frames_per_block: slice.frames_per_block,
            channels: slice
                .channels
                .into_iter()
                .map(|channel| {
                    channel
                        .into_iter()
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
