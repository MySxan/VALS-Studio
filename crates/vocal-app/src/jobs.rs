use super::*;
use vocal_analysis_api::{AnalysisContext, AnalysisError, ProgressSink};
use vocal_audio::{AudioError, DecodeAnalyzer, WavImporter};
use vocal_domain::audio::{AudioSource, ChannelMode, VocalTrack};
use vocal_dsp::WaveformAnalyzer;

struct Failure {
    status: SourceStatus,
    message: String,
}
impl From<AudioError> for Failure {
    fn from(error: AudioError) -> Self {
        let status = match &error {
            AudioError::SourceChanged => SourceStatus::Changed,
            AudioError::Io(e) if e.kind() == std::io::ErrorKind::NotFound => SourceStatus::Offline,
            AudioError::Invalid("source location requires relinking on this host") => {
                SourceStatus::Offline
            }
            _ => SourceStatus::Error,
        };
        Self {
            status,
            message: error.to_string(),
        }
    }
}
impl From<AnalysisError> for Failure {
    fn from(error: AnalysisError) -> Self {
        Self {
            status: if matches!(error, AnalysisError::SourceChanged) {
                SourceStatus::Changed
            } else {
                SourceStatus::Error
            },
            message: error.to_string(),
        }
    }
}
enum Work {
    Import(PathBuf),
    Analyze(AudioSource, Option<PathBuf>),
    Relink(AudioSource, PathBuf),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Publication {
    Attach,
    Analyze,
    Relink,
}
struct Candidate {
    source: AudioSource,
    track: VocalTrack,
    artifact: Arc<AnalysisArtifact>,
    publication: Publication,
}
impl AppService {
    pub fn start_import(
        &self,
        project_id: &str,
        generation: u64,
        path: PathBuf,
    ) -> Result<String, AppError> {
        self.start(project_id, generation, None, Work::Import(path))
    }
    pub fn analyze_track(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
    ) -> Result<String, AppError> {
        let (source, track, path) = self.resolve_track(project_id, generation, track_id)?;
        self.start(
            project_id,
            generation,
            Some(track),
            Work::Analyze(source, path),
        )
    }
    /// Verify an explicit same-content candidate and atomically publish its location.
    pub fn relink_track(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
        path: PathBuf,
    ) -> Result<String, AppError> {
        let (source, track, _) = self.resolve_track(project_id, generation, track_id)?;
        self.start(
            project_id,
            generation,
            Some(track),
            Work::Relink(source, path),
        )
    }
    fn resolve_track(
        &self,
        project_id: &str,
        generation: u64,
        track_id: &str,
    ) -> Result<(AudioSource, VocalTrack, Option<PathBuf>), AppError> {
        let state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        let project = state.project(project_id, generation)?;
        let track = project
            .domain
            .tracks()
            .iter()
            .find(|t| t.id.to_string() == track_id)
            .ok_or(AppError::UnknownTrack)?
            .clone();
        let source = project
            .domain
            .sources()
            .iter()
            .find(|s| s.id() == track.source)
            .ok_or(AppError::UnknownTrack)?
            .clone();
        Ok((source, track, project.path.clone()))
    }
    fn start(
        &self,
        project_id: &str,
        generation: u64,
        track: Option<VocalTrack>,
        work: Work,
    ) -> Result<String, AppError> {
        let mut state = self.0.state.lock().map_err(|_| AppError::Unavailable)?;
        let project = state.project(project_id, generation)?;
        // Save As or Relink can complete between identity lookup and job admission.
        let work = match work {
            Work::Analyze(source, _) => Work::Analyze(
                project
                    .domain
                    .sources()
                    .iter()
                    .find(|s| s.id() == source.id())
                    .ok_or(AppError::UnknownTrack)?
                    .clone(),
                project.path.clone(),
            ),
            other => other,
        };
        state.idle()?;
        self.0
            .playback
            .lock()
            .map_err(|_| AppError::Unavailable)?
            .clear();
        let id = EntityId::new().to_string();
        let token = CancellationToken::default();
        let track_id = track.as_ref().map(|t| t.id.to_string());
        let service = self.clone();
        let worker_id = id.clone();
        let worker_token = token.clone();
        let relink = matches!(&work, Work::Relink(..));
        std::thread::Builder::new()
            .name("vocal-analysis".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    service.run(work, track, &worker_token)
                }));
                service.finish(
                    &worker_id,
                    relink,
                    result.unwrap_or_else(|_| {
                        Err(Failure {
                            status: SourceStatus::Error,
                            message: "analysis.worker_panicked".into(),
                        })
                    }),
                );
            })
            .map_err(|_| AppError::Unavailable)?;
        if let Some(track_id) = track_id.as_ref().filter(|_| !relink) {
            let id = track_id.parse().map_err(|_| AppError::UnknownTrack)?;
            state
                .project
                .as_mut()
                .ok_or(AppError::StaleProject)?
                .derived
                .insert(
                    id,
                    DerivedTrack {
                        status: SourceStatus::Analyzing,
                        ..Default::default()
                    },
                );
        }
        state.job = Some((
            JobStatus {
                id: id.clone(),
                project_id: project_id.into(),
                generation,
                track_id,
                phase: JobPhase::Running,
                error: None,
            },
            token,
        ));
        Ok(id)
    }
    fn run(
        &self,
        work: Work,
        track: Option<VocalTrack>,
        token: &CancellationToken,
    ) -> Result<Candidate, Failure> {
        let (source, track, publication) = match work {
            Work::Import(path) => {
                let source = WavImporter::import_linked(&path, token)?;
                let track = VocalTrack {
                    id: EntityId::new(),
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    source: source.id(),
                    channel_mode: ChannelMode::EqualPowerMono,
                };
                (source, track, Publication::Attach)
            }
            Work::Relink(original, path) => {
                let candidate = WavImporter::import_linked(&path, token)?;
                if candidate.content_hash() != original.content_hash()
                    || candidate.size_bytes() != original.size_bytes()
                    || candidate.metadata() != original.metadata()
                {
                    return Err(Failure {
                        status: SourceStatus::Changed,
                        message: "relink.candidate_changed: 候选文件与原音频内容不同，关联未修改"
                            .into(),
                    });
                }
                let source = AudioSource::new(
                    original.id(),
                    candidate.uri().clone(),
                    original.content_hash(),
                    original.size_bytes(),
                    original.metadata(),
                )
                .expect("validated candidate location and original source facts");
                (source, track.expect("validated track"), Publication::Relink)
            }
            Work::Analyze(source, path) => {
                let source = paths::resolve(source, path.as_deref(), token)?;
                (
                    source,
                    track.expect("validated track"),
                    Publication::Analyze,
                )
            }
        };
        let mut runtime = self.0.runtime.lock().map_err(|_| Failure {
            status: SourceStatus::Error,
            message: "analysis.runtime_unavailable".into(),
        })?;
        let mut context = AnalysisContext::new(source.clone());
        let decoded = runtime.execute(
            &DecodeAnalyzer,
            context.clone(),
            token.clone(),
            ProgressSink::default(),
        )?;
        context.dependencies.push(decoded.artifact);
        let artifact = runtime
            .execute(
                &WaveformAnalyzer,
                context,
                token.clone(),
                ProgressSink::default(),
            )?
            .artifact;
        Ok(Candidate {
            source,
            track,
            artifact,
            publication,
        })
    }
    fn finish(&self, id: &str, relink: bool, result: Result<Candidate, Failure>) {
        let Ok(mut state) = self.0.state.lock() else {
            return;
        };
        let Some((job, token)) = state.job.as_ref() else {
            return;
        };
        if job.id != id || job.phase.is_terminal() {
            return;
        }
        let job = job.clone();
        let cancelled = token.is_cancelled();
        if state.project(&job.project_id, job.generation).is_err() {
            return;
        }
        let project = state.project.as_mut().expect("checked project");
        let (phase, error, track_id) = if cancelled {
            if let Some(id) = job.track_id.as_ref().filter(|_| !relink) {
                if let Ok(id) = id.parse() {
                    project.derived.insert(id, DerivedTrack::default());
                }
            }
            (JobPhase::Cancelled, None, job.track_id)
        } else {
            match result {
                Ok(candidate) => {
                    if candidate.publication == Publication::Attach {
                        if let Err(error) = project
                            .domain
                            .attach_audio(candidate.source.clone(), candidate.track.clone())
                        {
                            let job = &mut state.job.as_mut().expect("checked job").0;
                            job.phase = JobPhase::Failed;
                            job.error = Some(error.to_string());
                            return;
                        }
                        project.dirty = true;
                    }
                    if candidate.publication == Publication::Relink {
                        let uri = paths::uri_for_project(
                            &paths::fallback(&candidate.source),
                            project.path.as_deref(),
                        )
                        .expect("importer validates candidate location as UTF-8");
                        let changed = project
                            .domain
                            .sources()
                            .iter()
                            .find(|s| s.id() == candidate.source.id())
                            .is_some_and(|s| s.uri() != &uri);
                        if let Err(error) =
                            project.domain.set_source_uri(candidate.source.id(), uri)
                        {
                            let job = &mut state.job.as_mut().expect("checked job").0;
                            job.phase = JobPhase::Failed;
                            job.error = Some(error.to_string());
                            return;
                        }
                        project.dirty |= changed;
                        // Every track referencing this source observes the same location recovery.
                        for track in project
                            .domain
                            .tracks()
                            .iter()
                            .filter(|t| t.source == candidate.source.id())
                        {
                            project.derived.insert(
                                track.id,
                                DerivedTrack {
                                    status: SourceStatus::Ready,
                                    error: None,
                                    artifact: Some(candidate.artifact.clone()),
                                    resolved_path: Some(paths::fallback(&candidate.source)),
                                },
                            );
                        }
                    }
                    project.derived.insert(
                        candidate.track.id,
                        DerivedTrack {
                            status: SourceStatus::Ready,
                            error: None,
                            resolved_path: Some(paths::fallback(&candidate.source)),
                            artifact: Some(candidate.artifact),
                        },
                    );
                    (
                        JobPhase::Succeeded,
                        None,
                        Some(candidate.track.id.to_string()),
                    )
                }
                Err(error) => {
                    if let Some(id) = job.track_id.as_ref().filter(|_| !relink) {
                        if let Ok(id) = id.parse() {
                            project.derived.insert(
                                id,
                                DerivedTrack {
                                    status: error.status,
                                    error: Some(error.message.clone()),
                                    artifact: None,
                                    resolved_path: None,
                                },
                            );
                        }
                    }
                    (JobPhase::Failed, Some(error.message), job.track_id)
                }
            }
        };
        let job = &mut state.job.as_mut().expect("checked job").0;
        job.phase = phase;
        job.error = error;
        job.track_id = track_id;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_at_commit_prevents_partial_attachment() {
        let service = AppService::default();
        let snapshot = service.new_project("test".into(), 0, false).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        service
            .save_project(
                &snapshot.project.as_ref().unwrap().id,
                snapshot.generation,
                Some(tmp.path().join("empty.vocalproj")),
            )
            .unwrap();
        let token = CancellationToken::default();
        let candidate = service
            .run(Work::Import(super::super::tests::fixture()), None, &token)
            .ok()
            .unwrap();
        token.cancel();
        service.0.state.lock().unwrap().job = Some((
            JobStatus {
                id: "job".into(),
                project_id: snapshot.project.unwrap().id,
                generation: snapshot.generation,
                track_id: None,
                phase: JobPhase::Running,
                error: None,
            },
            token,
        ));
        service.finish("job", false, Ok(candidate));
        let state = service.0.state.lock().unwrap();
        let project = state.project.as_ref().unwrap();
        assert!(project.domain.sources().is_empty());
        assert!(project.domain.tracks().is_empty());
        assert!(!project.dirty);
        assert_eq!(state.job.as_ref().unwrap().0.phase, JobPhase::Cancelled);
    }
    #[test]
    fn cancelled_relink_at_commit_preserves_project_and_derived_evidence() {
        let service = AppService::default();
        let snapshot = service.new_project("test".into(), 0, false).unwrap();
        let token = CancellationToken::default();
        let candidate = service
            .run(Work::Import(super::super::tests::fixture()), None, &token)
            .ok()
            .unwrap();
        let source = candidate.source.clone();
        let track = candidate.track.clone();
        {
            let mut state = service.0.state.lock().unwrap();
            let p = state.project.as_mut().unwrap();
            p.domain
                .attach_audio(source.clone(), track.clone())
                .unwrap();
            p.dirty = false;
            p.derived.insert(
                track.id,
                DerivedTrack {
                    status: SourceStatus::Ready,
                    error: None,
                    resolved_path: Some(paths::fallback(&source)),
                    artifact: Some(candidate.artifact),
                },
            );
        }
        let before = serde_json::to_value(service.current_project().unwrap().project).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let relocated = tmp.path().join("relocated.wav");
        std::fs::copy(super::super::tests::fixture(), &relocated).unwrap();
        let candidate = service
            .run(Work::Relink(source, relocated), Some(track.clone()), &token)
            .ok()
            .unwrap();
        token.cancel();
        service.0.state.lock().unwrap().job = Some((
            JobStatus {
                id: "relink".into(),
                project_id: snapshot.project.unwrap().id,
                generation: snapshot.generation,
                track_id: Some(track.id.to_string()),
                phase: JobPhase::Cancelling,
                error: None,
            },
            token,
        ));
        service.finish("relink", true, Ok(candidate));
        assert_eq!(
            service.job_status("relink").unwrap().phase,
            JobPhase::Cancelled
        );
        assert_eq!(
            serde_json::to_value(service.current_project().unwrap().project).unwrap(),
            before
        );
    }
    #[test]
    fn stale_worker_cannot_publish_into_another_project() {
        let service = AppService::default();
        let old = service.new_project("old".into(), 0, false).unwrap();
        let candidate = service
            .run(
                Work::Import(super::super::tests::fixture()),
                None,
                &CancellationToken::default(),
            )
            .ok()
            .unwrap();
        let new = service
            .new_project("new".into(), old.generation, true)
            .unwrap();
        service.0.state.lock().unwrap().job = Some((
            JobStatus {
                id: "new-job".into(),
                project_id: new.project.unwrap().id,
                generation: new.generation,
                track_id: None,
                phase: JobPhase::Running,
                error: None,
            },
            CancellationToken::default(),
        ));
        service.finish("old-job", false, Ok(candidate));
        let state = service.0.state.lock().unwrap();
        assert_eq!(state.project.as_ref().unwrap().domain.name(), "new");
        assert!(state.project.as_ref().unwrap().domain.tracks().is_empty());
        assert_eq!(state.job.as_ref().unwrap().0.phase, JobPhase::Running);
    }
}
