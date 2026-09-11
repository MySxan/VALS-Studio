use crate::{
    format::{Manifest, ProjectV1, ProjectV2},
    MigrationRegistry, ProjectError, MAX_JSON_BYTES, SCHEMA_VERSION,
};
use std::{
    fs::File,
    io::{Read, Seek, Write},
    path::Path,
};
use tempfile::NamedTempFile;
use vocal_domain::project::VocalProject;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

#[derive(Default)]
pub struct ProjectStore {
    migrations: MigrationRegistry,
}

impl ProjectStore {
    pub fn new(migrations: MigrationRegistry) -> Self {
        Self { migrations }
    }

    pub fn load(&self, path: impl AsRef<Path>) -> Result<VocalProject, ProjectError> {
        self.read(File::open(path)?)
    }

    fn read(&self, file: File) -> Result<VocalProject, ProjectError> {
        // Bound the container before its central directory is parsed.
        if file.metadata()?.len() > MAX_JSON_BYTES * 2 + 65536 {
            return Err(ProjectError::Invalid(
                "project container exceeds minimal schema limit",
            ));
        }
        let mut archive = ZipArchive::new(file)?;
        if archive.len() != 2
            || archive
                .file_names()
                .any(|name| !matches!(name, "manifest.json" | "project.json"))
        {
            return Err(ProjectError::Invalid(
                "unsupported or missing archive entries",
            ));
        }
        let manifest: Manifest =
            serde_json::from_slice(&read_entry(&mut archive, "manifest.json")?)?;
        let id = manifest.validate()?;
        if manifest.schema_version > SCHEMA_VERSION {
            return Err(ProjectError::UnsupportedVersion(manifest.schema_version));
        }
        let bytes = read_entry(&mut archive, "project.json")?;
        // Deserialize current schema directly to reject duplicate as well as unknown fields.
        let dto: ProjectV2 = if manifest.schema_version == SCHEMA_VERSION {
            serde_json::from_slice(&bytes)?
        } else {
            // Validate v1 directly before Value parsing can collapse duplicate fields.
            if manifest.schema_version == 1 {
                let _: ProjectV1 = serde_json::from_slice(&bytes)?;
            }
            let raw = self
                .migrations
                .upgrade(manifest.schema_version, serde_json::from_slice(&bytes)?)?;
            serde_json::from_value(raw)?
        };
        dto.into_domain(id)
    }

    /// Explicit last-writer-wins save. Callers must serialize saves for a project.
    /// Atomic replacement protects against partial files, not concurrent edit conflicts.
    pub fn save(&self, path: impl AsRef<Path>, project: &VocalProject) -> Result<(), ProjectError> {
        let path = path.as_ref();
        let manifest = serde_json::to_vec_pretty(&Manifest::new(project))?;
        let payload = serde_json::to_vec_pretty(&ProjectV2::from(project))?;
        if manifest.len() as u64 > MAX_JSON_BYTES || payload.len() as u64 > MAX_JSON_BYTES {
            return Err(ProjectError::Invalid("project JSON exceeds size limit"));
        }
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temp = NamedTempFile::new_in(parent)?;
        {
            let mut writer = ZipWriter::new(temp.as_file_mut());
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for (name, bytes) in [("manifest.json", manifest), ("project.json", payload)] {
                writer.start_file(name, options)?;
                writer.write_all(&bytes)?;
            }
            writer.finish()?;
        }
        temp.as_file().sync_all()?;
        let validated = self.read(temp.reopen()?)?;
        if &validated != project {
            return Err(ProjectError::Invalid("saved project validation mismatch"));
        }
        // tempfile handles platform-specific atomic replacement, including Windows.
        temp.persist(path).map_err(|e| ProjectError::Io(e.error))?;
        Ok(())
    }
}

fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, ProjectError> {
    let entry = archive.by_name(name)?;
    if entry.size() > MAX_JSON_BYTES || entry.compression() != CompressionMethod::Stored {
        return Err(ProjectError::Invalid(
            "oversized or unsupported compressed JSON entry",
        ));
    }
    let mut bytes = Vec::new();
    entry.take(MAX_JSON_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_JSON_BYTES {
        return Err(ProjectError::Invalid("JSON entry exceeds size limit"));
    }
    Ok(bytes)
}
