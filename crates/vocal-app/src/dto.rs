use crate::OpenProject;
use serde::Serialize;
use std::collections::BTreeMap;
use vocal_analysis_api::AnalysisArtifact;
use vocal_domain::analysis::ConfidenceKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JobPhase {
    Running,
    Cancelling,
    Succeeded,
    Cancelled,
    Failed,
}
impl JobPhase {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Cancelled | Self::Failed)
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub id: String,
    pub project_id: String,
    pub generation: u64,
    pub track_id: Option<String>,
    pub phase: JobPhase,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    pub id: String,
    pub name: String,
    pub source_id: String,
    pub source_path: String,
    pub source_hash: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: f64,
    pub status: SourceStatus,
    pub error: Option<String>,
    pub analysis: Option<AnalysisDto>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceStatus {
    #[default]
    Unchecked,
    Analyzing,
    Ready,
    Offline,
    Changed,
    Error,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    pub generation: u64,
    pub project: Option<ProjectDto>,
    pub job: Option<JobStatus>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
    pub path: Option<String>,
    pub dirty: bool,
    pub tracks: Vec<TrackDto>,
}
impl ProjectDto {
    pub(crate) fn from_project(p: &OpenProject) -> Self {
        Self {
            id: p.domain.id().to_string(),
            name: p.domain.name().into(),
            path: p.path.as_ref().map(|v| v.to_string_lossy().into()),
            dirty: p.dirty,
            tracks: p
                .domain
                .tracks()
                .iter()
                .map(|t| {
                    let source = p
                        .domain
                        .sources()
                        .iter()
                        .find(|s| s.id() == t.source)
                        .expect("validated domain reference");
                    let derived = p.derived.get(&t.id);
                    TrackDto {
                        id: t.id.to_string(),
                        name: t.name.clone(),
                        source_id: t.source.to_string(),
                        source_path: derived
                            .and_then(|d| d.resolved_path.clone())
                            .or_else(|| crate::paths::relative_candidate(source, p.path.as_deref()))
                            .unwrap_or_else(|| crate::paths::fallback(source))
                            .to_string_lossy()
                            .into(),
                        source_hash: source.content_hash().to_string(),
                        sample_rate: source.metadata().sample_rate(),
                        channels: source.metadata().channels(),
                        duration: source.metadata().duration().get(),
                        status: derived.map(|d| d.status).unwrap_or_default(),
                        error: derived.and_then(|d| d.error.clone()),
                        analysis: derived.and_then(|d| {
                            d.artifact.as_ref().map(|a| AnalysisDto::from_artifact(a))
                        }),
                    }
                })
                .collect(),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisDto {
    pub artifact_hash: String,
    pub provenance: ProvenanceDto,
    pub confidence: ConfidenceDto,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceDto {
    pub kind: String,
    pub score: Option<f32>,
    pub explanation: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceDto {
    pub analyzer_id: String,
    pub analyzer_version: String,
    pub provider: String,
    pub model: Option<ModelDto>,
    pub settings: BTreeMap<String, String>,
    pub settings_hash: String,
    pub source_hash: String,
    pub dependency_hashes: Vec<String>,
    pub created_at_unix_ms: Option<u128>,
    pub runtime: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ModelDto {
    pub id: String,
    pub version: String,
    pub hash: String,
}
impl AnalysisDto {
    pub(crate) fn from_artifact(artifact: &AnalysisArtifact) -> Self {
        let p = artifact.provenance();
        let c = artifact.confidence();
        Self {
            artifact_hash: artifact.artifact_hash().to_string(),
            confidence: ConfidenceDto {
                kind: match c.kind() {
                    ConfidenceKind::Measurement => "measurement",
                    ConfidenceKind::ModelProbability => "modelProbability",
                    ConfidenceKind::Heuristic => "heuristic",
                }
                .into(),
                score: c.score(),
                explanation: c.explanation().into(),
            },
            provenance: ProvenanceDto {
                analyzer_id: p.analyzer_id.clone(),
                analyzer_version: p.analyzer_version.clone(),
                provider: p.provider.clone(),
                model: p.model.as_ref().map(|m| ModelDto {
                    id: m.id.clone(),
                    version: m.version.clone(),
                    hash: m.hash.to_string(),
                }),
                settings: p.settings.clone(),
                settings_hash: p.settings_hash.to_string(),
                source_hash: p.source_hash.to_string(),
                dependency_hashes: p
                    .dependency_hashes
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                created_at_unix_ms: p
                    .created_at
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_millis()),
                runtime: p.runtime.clone(),
            },
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformDto {
    pub project_id: String,
    pub generation: u64,
    pub track_id: String,
    pub artifact_hash: String,
    pub frames_per_block: u64,
    pub channels: Vec<Vec<PointDto>>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PointDto {
    pub start: i64,
    pub end: i64,
    pub min: f32,
    pub max: f32,
    pub rms: f64,
}
