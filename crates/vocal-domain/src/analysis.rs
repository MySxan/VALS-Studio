use crate::audio::{AudioDomainError, ContentHash};
use std::{collections::BTreeMap, time::SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceKind {
    Measurement,
    ModelProbability,
    Heuristic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Confidence {
    score: Option<f32>,
    kind: ConfidenceKind,
    explanation: String,
}
impl Confidence {
    /// None means a probability score is not applicable, never implicitly 1.0.
    pub fn new(
        score: Option<f32>,
        kind: ConfidenceKind,
        explanation: String,
    ) -> Result<Self, AudioDomainError> {
        if explanation.is_empty()
            || score.is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            || (kind == ConfidenceKind::Measurement && score.is_some())
        {
            return Err(AudioDomainError("invalid confidence semantics"));
        }
        Ok(Self {
            score,
            kind,
            explanation,
        })
    }
    pub fn score(&self) -> Option<f32> {
        self.score
    }
    pub fn kind(&self) -> ConfidenceKind {
        self.kind
    }
    pub fn explanation(&self) -> &str {
        &self.explanation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIdentity {
    pub id: String,
    pub version: String,
    pub hash: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub analyzer_id: String,
    pub analyzer_version: String,
    pub provider: String,
    pub model: Option<ModelIdentity>,
    pub settings: BTreeMap<String, String>,
    pub settings_hash: ContentHash,
    pub source_hash: ContentHash,
    pub dependency_hashes: Vec<ContentHash>,
    pub created_at: SystemTime,
    pub runtime: String,
}
