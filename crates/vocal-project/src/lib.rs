//! Versioned project ZIP persistence. Domain objects are mapped through private DTOs.
mod error;
mod format;
mod migration;
mod store;

pub use error::ProjectError;
pub use migration::{MigrationRegistry, ProjectMigration};
pub use store::ProjectStore;

pub const SCHEMA_VERSION: u32 = 2;
/// Per JSON entry, measured as UTF-8 bytes. This minimal schema contains no audio.
pub const MAX_JSON_BYTES: u64 = 1024 * 1024;
