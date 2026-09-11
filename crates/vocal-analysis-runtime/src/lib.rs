//! Synchronous single-node executor. Callers supply the explicit topological chain.
//! No UI thread scheduling or project mutation; parallel DAG scheduling comes later.
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use vocal_analysis_api::*;
use vocal_domain::audio::ContentHash;

pub struct AnalysisRuntime {
    cache: HashMap<ContentHash, Arc<AnalysisArtifact>>,
    insertion_order: VecDeque<ContentHash>,
    budget: usize,
    used: usize,
}
pub struct Execution {
    pub artifact: Arc<AnalysisArtifact>,
    pub cache_hit: bool,
}
impl AnalysisRuntime {
    /// Budget counts retained payload bytes; at most 64 entries. External Arc owners are independent.
    pub fn new(payload_budget: usize) -> Self {
        Self {
            cache: HashMap::new(),
            insertion_order: VecDeque::new(),
            budget: payload_budget,
            used: 0,
        }
    }
    pub fn cached_payload_bytes(&self) -> usize {
        self.used
    }
    pub fn cached_entries(&self) -> usize {
        self.cache.len()
    }
    pub fn execute(
        &mut self,
        analyzer: &dyn Analyzer,
        mut ctx: AnalysisContext,
        cancel: CancellationToken,
        progress: ProgressSink,
    ) -> Result<Execution, AnalysisError> {
        cancel.check()?;
        ctx.settings = analyzer.normalized_settings(&ctx)?;
        let descriptor = analyzer.descriptor();
        if analyzer.dependencies()
            != ctx
                .dependencies
                .iter()
                .map(|a| a.kind())
                .collect::<Vec<_>>()
            || ctx
                .dependencies
                .iter()
                .any(|a| a.provenance().source_hash != ctx.source.content_hash())
        {
            return Err(AnalysisError::Invalid(
                "missing, extra, reordered or wrong-source dependencies",
            ));
        }
        if analyzer.supports(&ctx) != SupportLevel::Supported {
            return Err(AnalysisError::Unsupported);
        }
        analyzer.validate_inputs(&ctx, &cancel)?;
        cancel.check()?;
        let key = cache_key(&descriptor, &ctx);
        if let Some(artifact) = self.cache.get(&key) {
            return Ok(Execution {
                artifact: artifact.clone(),
                cache_hit: true,
            });
        }
        let artifact = analyzer.run(ctx, cancel.clone(), progress)?;
        cancel.check()?;
        if artifact.kind() != descriptor.kind || artifact.cache_key() != key {
            return Err(AnalysisError::Invalid(
                "provider returned an artifact for different inputs",
            ));
        }
        let artifact = Arc::new(artifact);
        let bytes = artifact.payload_bytes();
        if bytes <= self.budget {
            while self.used > self.budget - bytes || self.cache.len() >= 64 {
                if let Some(old) = self.insertion_order.pop_front() {
                    if let Some(removed) = self.cache.remove(&old) {
                        self.used -= removed.payload_bytes();
                    }
                }
            }
            self.used += bytes;
            self.insertion_order.push_back(key);
            self.cache.insert(key, artifact.clone());
        }
        Ok(Execution {
            artifact,
            cache_hit: false,
        })
    }
}
