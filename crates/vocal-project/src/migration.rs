use crate::{ProjectError, SCHEMA_VERSION};
use serde_json::Value;
use std::collections::BTreeMap;

/// Migrations transform project.json in memory; they never mutate the source file.
/// A migration must preserve IDs, observations, provenance, and user overrides.
pub trait ProjectMigration: Send + Sync {
    // Preserve the explicit migration contract terminology from SDD §28.3.
    #[allow(clippy::wrong_self_convention)]
    fn from_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn migrate(&self, raw: Value) -> Result<Value, ProjectError>;
}

pub struct MigrationRegistry {
    steps: BTreeMap<u32, Box<dyn ProjectMigration>>,
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        let mut registry = Self::empty();
        registry.steps.insert(1, Box::new(V1ToV2));
        registry
    }
}

struct V1ToV2;
impl ProjectMigration for V1ToV2 {
    fn from_version(&self) -> u32 {
        1
    }
    fn to_version(&self) -> u32 {
        2
    }
    fn migrate(&self, raw: Value) -> Result<Value, ProjectError> {
        // Strict v1 validation prevents silently deleting unknown analysis/override data.
        let old: crate::format::ProjectV1 = serde_json::from_value(raw)?;
        Ok(serde_json::json!({"id":old.id,"name":old.name,"sources":[],"tracks":[]}))
    }
}

impl MigrationRegistry {
    /// Empty registry for controlled custom chains; production default includes v1 -> v2.
    pub fn empty() -> Self {
        Self {
            steps: BTreeMap::new(),
        }
    }
    pub fn register(&mut self, step: Box<dyn ProjectMigration>) -> Result<(), ProjectError> {
        let from = step.from_version();
        if from.checked_add(1) != Some(step.to_version())
            || step.to_version() > SCHEMA_VERSION
            || self.steps.contains_key(&from)
        {
            return Err(ProjectError::Migration(
                "migration must be a unique adjacent step up to the current schema".into(),
            ));
        }
        self.steps.insert(from, step);
        Ok(())
    }

    pub(crate) fn upgrade(&self, mut version: u32, mut raw: Value) -> Result<Value, ProjectError> {
        if version > SCHEMA_VERSION {
            return Err(ProjectError::UnsupportedVersion(version));
        }
        while version < SCHEMA_VERSION {
            let step = self.steps.get(&version).ok_or_else(|| {
                ProjectError::Migration(format!("missing migration from schema {version}"))
            })?;
            raw = step.migrate(raw)?;
            version += 1;
        }
        Ok(raw)
    }
}
