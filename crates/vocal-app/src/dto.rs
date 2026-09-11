use crate::Session;
use serde::Serialize;
use std::collections::BTreeMap;
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
    pub phase: JobPhase,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    pub id: String,
    pub source_hash: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration: f64,
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
impl SessionDto {
    pub(crate) fn from_session(s: &Session) -> Self {
        let p = s.waveform.provenance();
        let c = s.waveform.confidence();
        Self {
            id: s.id.clone(),
            source_hash: s.source.content_hash().to_string(),
            sample_rate: s.source.metadata().sample_rate(),
            channels: s.source.metadata().channels(),
            duration: s.source.metadata().duration().get(),
            artifact_hash: s.waveform.artifact_hash().to_string(),
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
    pub session_id: String,
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
