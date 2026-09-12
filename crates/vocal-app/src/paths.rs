//! Project-relative locations are application concerns; storage never resolves audio.
use super::*;
use std::path::{Component, Path};
use vocal_audio::{verify_source, AudioError};
use vocal_domain::audio::AudioSource;

pub(crate) fn absolute_project_path(path: &Path) -> Result<PathBuf, AppError> {
    let absolute = std::path::absolute(path).map_err(|e| AppError::Storage(e.to_string()))?;
    let parent = absolute.parent().ok_or(AppError::InvalidPath)?;
    let parent = parent
        .canonicalize()
        .map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(parent.join(absolute.file_name().ok_or(AppError::InvalidPath)?))
}

pub(crate) fn relative_candidate(source: &AudioSource, project: Option<&Path>) -> Option<PathBuf> {
    let AudioUri::Linked { relative_path, .. } = source.uri();
    let mut path = project?.parent()?.to_path_buf();
    // Domain validation rejects absolute paths, drive prefixes and parent traversal.
    // Both separator styles are interpreted independently of the host platform.
    for part in relative_path.as_ref()?.split(['/', '\\']) {
        path.push(part);
    }
    Some(path)
}

pub(crate) fn fallback(source: &AudioSource) -> PathBuf {
    let AudioUri::Linked {
        absolute_fallback, ..
    } = source.uri();
    absolute_fallback.into()
}

fn located(source: &AudioSource, path: &Path) -> Result<AudioSource, AudioError> {
    let absolute = path
        .to_str()
        .ok_or(AudioError::Invalid("source path must be UTF-8"))?;
    Ok(AudioSource::new(
        source.id(),
        AudioUri::Linked {
            absolute_fallback: absolute.into(),
            relative_path: None,
        },
        source.content_hash(),
        source.size_bytes(),
        source.metadata(),
    )
    .expect("validated source facts and nonempty location"))
}

pub(crate) fn resolve(
    source: AudioSource,
    project: Option<&Path>,
    token: &CancellationToken,
) -> Result<AudioSource, AudioError> {
    token.check()?;
    if let Some(candidate) = relative_candidate(&source, project) {
        match candidate.canonicalize() {
            Ok(path) => {
                let root = project
                    .and_then(Path::parent)
                    .expect("relative candidate has a root")
                    .canonicalize()?;
                if !path.starts_with(root) {
                    return Err(AudioError::Invalid(
                        "relative source escapes project directory",
                    ));
                }
                let resolved = located(&source, &path)?;
                // A changed, unreadable or invalid relative candidate is not silently bypassed.
                verify_source(&resolved, token)?;
                return Ok(resolved);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    verify_source(&source, token)?;
    located(&source, &fallback(&source).canonicalize()?)
}

/// Choose the last verified location, otherwise preserve the original relative target.
/// No source content is read and missing sources remain saveable.
pub(crate) fn saved_domain(
    project: &OpenProject,
    destination: &Path,
) -> Result<VocalProject, AppError> {
    let mut domain = project.domain.clone();
    let root = destination.parent().ok_or(AppError::InvalidPath)?;
    for source in project.domain.sources() {
        let verified = project
            .domain
            .tracks()
            .iter()
            .filter(|t| t.source == source.id())
            .find_map(|t| {
                project
                    .derived
                    .get(&t.id)
                    .and_then(|d| d.resolved_path.clone())
            });
        let relative = relative_candidate(source, project.path.as_deref());
        if verified.is_none()
            && relative.is_some()
            && project.path.as_deref().and_then(Path::parent) == Some(root)
        {
            continue; // Ordinary offline Save preserves both existing candidates exactly.
        }
        let location = verified.or(relative).unwrap_or_else(|| fallback(source));
        domain
            .set_source_uri(
                source.id(),
                uri_for_project(&location, Some(destination))
                    .map_err(|e| AppError::Storage(e.to_string()))?,
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
    }
    Ok(domain)
}

pub(crate) fn uri_for_project(
    location: &Path,
    project: Option<&Path>,
) -> Result<AudioUri, AudioError> {
    let absolute = location
        .to_str()
        .ok_or(AudioError::Invalid("source path must be UTF-8"))?;
    let relative_path = project
        .and_then(Path::parent)
        .and_then(|root| location.strip_prefix(root).ok())
        .and_then(|p| {
            let parts = p
                .components()
                .map(|part| match part {
                    Component::Normal(name) => name.to_str(),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            (!parts.is_empty()).then(|| parts.join("/"))
        });
    Ok(AudioUri::Linked {
        absolute_fallback: absolute.into(),
        relative_path,
    })
}
