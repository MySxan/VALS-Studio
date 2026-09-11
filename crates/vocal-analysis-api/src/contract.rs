use crate::AnalysisArtifact;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use vocal_domain::{
    analysis::ModelIdentity,
    audio::{AudioSource, ContentHash},
};

pub const MAX_PCM_BYTES: u64 = 64 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnalysisKind {
    Decode,
    WaveformPeaks,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyzerDescriptor {
    pub id: String,
    pub version: String,
    pub provider: String,
    pub model: Option<ModelIdentity>,
    pub kind: AnalysisKind,
    pub deterministic: bool,
    pub hardware: HardwareRequirements,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardwareRequirements {
    pub cpu_threads: u16,
    pub exclusive_gpu: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportLevel {
    Supported,
    Unsupported,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisError {
    Cancelled,
    Unsupported,
    Invalid(&'static str),
    LimitExceeded,
    Provider(String),
    SourceChanged,
}
impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AnalysisError {}

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
    pub fn check(&self) -> Result<(), AnalysisError> {
        if self.is_cancelled() {
            Err(AnalysisError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
pub struct AnalysisContext {
    pub source: AudioSource,
    /// Canonical string values: providers reject unknown/noncanonical settings.
    pub settings: BTreeMap<String, String>,
    pub dependencies: Vec<Arc<AnalysisArtifact>>,
}
impl AnalysisContext {
    pub fn new(source: AudioSource) -> Self {
        Self {
            source,
            settings: BTreeMap::new(),
            dependencies: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Progress {
    pub completed: u64,
    pub total: u64,
}
#[derive(Clone)]
pub struct ProgressSink(Arc<dyn Fn(Progress) + Send + Sync>);
impl Default for ProgressSink {
    fn default() -> Self {
        Self(Arc::new(|_| {}))
    }
}
impl ProgressSink {
    pub fn new(callback: impl Fn(Progress) + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }
    pub fn report(&self, completed: u64, total: u64) {
        (self.0)(Progress { completed, total });
    }
}

pub trait Analyzer: Send + Sync {
    fn descriptor(&self) -> AnalyzerDescriptor;
    fn dependencies(&self) -> Vec<AnalysisKind>;
    fn supports(&self, ctx: &AnalysisContext) -> SupportLevel;
    fn normalized_settings(
        &self,
        ctx: &AnalysisContext,
    ) -> Result<BTreeMap<String, String>, AnalysisError> {
        Ok(ctx.settings.clone())
    }
    /// Called even on cache hits; linked-file providers must check source freshness.
    fn validate_inputs(
        &self,
        _ctx: &AnalysisContext,
        cancel: &CancellationToken,
    ) -> Result<(), AnalysisError> {
        cancel.check()
    }
    fn run(
        &self,
        ctx: AnalysisContext,
        cancel: CancellationToken,
        progress: ProgressSink,
    ) -> Result<AnalysisArtifact, AnalysisError>;
}

// All fields are length-delimited; map keys are lexically ordered. Paths and IDs are excluded.
pub(crate) struct HashBuilder(sha2::Sha256);
impl HashBuilder {
    pub fn new(tag: &str) -> Self {
        use sha2::Digest;
        let mut h = Self(sha2::Sha256::new());
        h.bytes(tag.as_bytes());
        h
    }
    pub fn bytes(&mut self, bytes: &[u8]) {
        use sha2::Digest;
        self.0.update((bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
    }
    pub fn finish(self) -> ContentHash {
        use sha2::Digest;
        ContentHash(self.0.finalize().into())
    }
}
pub fn settings_hash(settings: &BTreeMap<String, String>) -> ContentHash {
    let mut h = HashBuilder::new("settings-v1");
    for (k, v) in settings {
        h.bytes(k.as_bytes());
        h.bytes(v.as_bytes());
    }
    h.finish()
}
pub fn cache_key(descriptor: &AnalyzerDescriptor, ctx: &AnalysisContext) -> ContentHash {
    let mut h = HashBuilder::new("analysis-key-v1");
    for s in [&descriptor.id, &descriptor.version, &descriptor.provider] {
        h.bytes(s.as_bytes());
    }
    h.bytes(&[descriptor.kind as u8, u8::from(descriptor.deterministic)]);
    h.bytes(&[u8::from(descriptor.model.is_some())]);
    if let Some(m) = &descriptor.model {
        h.bytes(m.id.as_bytes());
        h.bytes(m.version.as_bytes());
        h.bytes(&m.hash.0);
    }
    h.bytes(&ctx.source.content_hash().0);
    h.bytes(&ctx.source.size_bytes().to_le_bytes());
    let m = ctx.source.metadata();
    h.bytes(&m.sample_rate().to_le_bytes());
    h.bytes(&m.channels().to_le_bytes());
    h.bytes(&m.frames().get().to_le_bytes());
    h.bytes(&settings_hash(&ctx.settings).0);
    for artifact in &ctx.dependencies {
        h.bytes(&artifact.artifact_hash().0);
    }
    h.finish()
}
