use super::*;
use vocal_analysis_api::AnalysisError;
use vocal_audio::{decode_verified_pcm, AudioError};
use vocal_domain::{audio::ContentHash, signal::Pcm};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackPhase {
    Stopped,
    Paused,
    Playing,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackEngineStatus {
    pub phase: PlaybackPhase,
    pub position_frames: u64,
    pub total_frames: u64,
}

/// Provider-independent playback port. Concrete device APIs live in desktop adapters.
pub trait PlaybackEngine: Send {
    fn load(&mut self, pcm: Pcm) -> Result<(), String>;
    fn play(&mut self) -> Result<(), String>;
    fn pause(&mut self) -> Result<(), String>;
    fn seek(&mut self, frame: u64) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
    fn status(&self) -> Result<PlaybackEngineStatus, String>;
}

pub(crate) struct PlaybackSession {
    pub engine: Box<dyn PlaybackEngine>,
    binding: Option<PlaybackBinding>,
}

#[derive(Clone)]
struct PlaybackBinding {
    project_id: String,
    generation: u64,
    track_id: String,
    source_hash: ContentHash,
    sample_rate: u32,
}

pub(crate) struct DisabledPlayback;

impl PlaybackEngine for DisabledPlayback {
    fn load(&mut self, _pcm: Pcm) -> Result<(), String> {
        Err("当前运行环境没有音频输出设备适配器".into())
    }
    fn play(&mut self) -> Result<(), String> {
        Err("播放尚未载入".into())
    }
    fn pause(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn seek(&mut self, _frame: u64) -> Result<(), String> {
        Err("播放尚未载入".into())
    }
    fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn status(&self) -> Result<PlaybackEngineStatus, String> {
        Err("播放尚未载入".into())
    }
}

impl PlaybackSession {
    pub(crate) fn new(engine: impl PlaybackEngine + 'static) -> Self {
        Self {
            engine: Box::new(engine),
            binding: None,
        }
    }

    pub(crate) fn clear(&mut self) {
        let _ = self.engine.stop();
        self.binding = None;
    }

    fn binding(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
    ) -> Result<&PlaybackBinding, AppError> {
        self.binding
            .as_ref()
            .filter(|binding| {
                binding.project_id == project_id
                    && binding.generation == generation
                    && binding.track_id == track_id
            })
            .ok_or(AppError::NotReady)
    }

    fn dto(&self, binding: &PlaybackBinding) -> Result<PlaybackDto, AppError> {
        let status = self.engine.status().map_err(AppError::Playback)?;
        let total_frames = status.total_frames;
        Ok(PlaybackDto {
            project_id: binding.project_id.clone(),
            generation: binding.generation,
            track_id: binding.track_id.clone(),
            phase: status.phase,
            position: status.position_frames.min(total_frames) as f64 / binding.sample_rate as f64,
            duration: total_frames as f64 / binding.sample_rate as f64,
        })
    }
}

impl AppService {
    pub fn with_playback(engine: impl PlaybackEngine + 'static) -> Self {
        Self(Arc::new(Inner {
            state: Mutex::new(State::default()),
            runtime: Mutex::new(AnalysisRuntime::new(128 * 1024 * 1024)),
            playback: Mutex::new(PlaybackSession::new(engine)),
        }))
    }

    pub fn load_playback(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
        position: f64,
    ) -> Result<PlaybackDto, AppError> {
        let (source, project_path) = {
            let state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
            let project = state.project(project_id, generation)?;
            let track = project
                .domain
                .tracks()
                .iter()
                .find(|track| track.id.to_string() == track_id)
                .ok_or(AppError::UnknownTrack)?;
            if project.derived.get(&track.id).map(|d| d.status) != Some(SourceStatus::Ready) {
                return Err(AppError::NotReady);
            }
            let source = project
                .domain
                .sources()
                .iter()
                .find(|source| source.id() == track.source)
                .ok_or(AppError::UnknownTrack)?
                .clone();
            (source, project.path.clone())
        };
        let position = Seconds::new(position).map_err(|_| AppError::InvalidPlaybackPosition)?;
        if position.get() < 0.0 {
            return Err(AppError::InvalidPlaybackPosition);
        }
        let token = CancellationToken::default();
        let source = match paths::resolve(source, project_path.as_deref(), &token) {
            Ok(source) => source,
            Err(error) => {
                let status = match &error {
                    AudioError::SourceChanged => SourceStatus::Changed,
                    AudioError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
                        SourceStatus::Offline
                    }
                    AudioError::Invalid("source location requires relinking on this host") => {
                        SourceStatus::Offline
                    }
                    _ => SourceStatus::Error,
                };
                let message = error.to_string();
                self.invalidate_playback_source(
                    project_id, generation, track_id, status, &message,
                )?;
                return Err(AppError::Playback(message));
            }
        };
        let pcm = match decode_verified_pcm(&source, &token) {
            Ok(pcm) => pcm,
            Err(error) => {
                let status = if matches!(error, AnalysisError::SourceChanged) {
                    SourceStatus::Changed
                } else {
                    SourceStatus::Error
                };
                let message = error.to_string();
                self.invalidate_playback_source(
                    project_id, generation, track_id, status, &message,
                )?;
                return Err(AppError::Playback(message));
            }
        };
        let metadata = pcm.metadata();
        let duration = metadata.duration().get();
        if position.get() > duration {
            return Err(AppError::InvalidPlaybackPosition);
        }

        // Recheck identity while holding state, then acquire playback in the
        // global state -> playback order used by all transitions.
        let state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        let project = state.project(project_id, generation)?;
        let track = project
            .domain
            .tracks()
            .iter()
            .find(|track| track.id.to_string() == track_id)
            .ok_or(AppError::UnknownTrack)?;
        let current_source = project
            .domain
            .sources()
            .iter()
            .find(|candidate| candidate.id() == track.source)
            .ok_or(AppError::UnknownTrack)?;
        if current_source.content_hash() != source.content_hash()
            || project.derived.get(&track.id).map(|d| d.status) != Some(SourceStatus::Ready)
        {
            return Err(AppError::StaleProject);
        }
        let mut playback = self.0.playback.lock().map_err(|_| AppError::Unavailable)?;
        playback.clear();
        playback.engine.load(pcm).map_err(AppError::Playback)?;
        let frame = (position.get() * metadata.sample_rate() as f64).round() as u64;
        if let Err(error) = playback.engine.seek(frame) {
            playback.clear();
            return Err(AppError::Playback(error));
        }
        playback.binding = Some(PlaybackBinding {
            project_id: project_id.into(),
            generation,
            track_id: track_id.into(),
            source_hash: source.content_hash(),
            sample_rate: metadata.sample_rate(),
        });
        let binding = playback.binding.as_ref().expect("binding just installed");
        playback.dto(binding)
    }

    pub fn play(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
    ) -> Result<PlaybackDto, AppError> {
        self.playback_command(project_id, generation, track_id, |engine, _| engine.play())
    }

    pub fn pause(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
    ) -> Result<PlaybackDto, AppError> {
        self.playback_command(project_id, generation, track_id, |engine, _| engine.pause())
    }

    pub fn seek(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
        position: f64,
    ) -> Result<PlaybackDto, AppError> {
        let position = Seconds::new(position).map_err(|_| AppError::InvalidPlaybackPosition)?;
        if position.get() < 0.0 {
            return Err(AppError::InvalidPlaybackPosition);
        }
        self.playback_command(project_id, generation, track_id, |engine, binding| {
            let status = engine.status()?;
            let frame = (position.get() * binding.sample_rate as f64).round() as u64;
            if frame > status.total_frames {
                return Err("播放位置超出音频范围".into());
            }
            engine.seek(frame)
        })
    }

    pub fn stop(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
    ) -> Result<PlaybackDto, AppError> {
        self.playback_command(project_id, generation, track_id, |engine, _| engine.stop())
    }

    pub fn playback_status(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
    ) -> Result<PlaybackDto, AppError> {
        self.playback_command(project_id, generation, track_id, |_engine, _| Ok(()))
    }

    fn playback_command(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
        command: impl FnOnce(&mut dyn PlaybackEngine, &PlaybackBinding) -> Result<(), String>,
    ) -> Result<PlaybackDto, AppError> {
        let state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        let project = state.project(project_id, generation)?;
        let track = project
            .domain
            .tracks()
            .iter()
            .find(|track| track.id.to_string() == track_id)
            .ok_or(AppError::UnknownTrack)?;
        let source = project
            .domain
            .sources()
            .iter()
            .find(|source| source.id() == track.source)
            .ok_or(AppError::UnknownTrack)?;
        let mut playback = self.0.playback.lock().map_err(|_| AppError::Unavailable)?;
        let binding = playback.binding(project_id, generation, track_id)?.clone();
        if binding.source_hash != source.content_hash() {
            playback.clear();
            return Err(AppError::StaleProject);
        }
        command(playback.engine.as_mut(), &binding).map_err(AppError::Playback)?;
        playback.dto(&binding)
    }

    fn invalidate_playback_source(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
        status: SourceStatus,
        message: &str,
    ) -> Result<(), AppError> {
        let mut state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        let project = state.project(project_id, generation)?;
        let id = track_id
            .parse::<EntityId>()
            .map_err(|_| AppError::UnknownTrack)?;
        if !project.domain.tracks().iter().any(|track| track.id == id) {
            return Err(AppError::UnknownTrack);
        }
        state
            .project
            .as_mut()
            .expect("checked project")
            .derived
            .insert(
                id,
                DerivedTrack {
                    status,
                    error: Some(message.into()),
                    artifact: None,
                    resolved_path: None,
                },
            );
        self.0
            .playback
            .lock()
            .map_err(|_| AppError::Unavailable)?
            .clear();
        Ok(())
    }
}
