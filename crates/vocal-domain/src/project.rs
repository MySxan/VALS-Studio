use crate::audio::{AudioDomainError, AudioSource, AudioUri, VocalTrack};
use crate::identity::EntityId;
use std::collections::HashSet;

/// Minimal project identity. Musical entities will be added in later slices.
/// No storage DTOs, generated analysis, or UI state belong here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VocalProject {
    id: EntityId,
    name: String,
    sources: Vec<AudioSource>,
    tracks: Vec<VocalTrack>,
}

impl VocalProject {
    pub fn new(name: impl Into<String>) -> Self {
        Self::from_parts(EntityId::new(), name)
    }

    pub fn from_parts(id: EntityId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            sources: Vec::new(),
            tracks: Vec::new(),
        }
    }

    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn rename(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn sources(&self) -> &[AudioSource] {
        &self.sources
    }
    pub fn tracks(&self) -> &[VocalTrack] {
        &self.tracks
    }

    /// Changes location only. The application must verify candidate content first.
    /// Identity, source facts, tracks and all other project data remain untouched.
    pub fn set_source_uri(&mut self, id: EntityId, uri: AudioUri) -> Result<(), AudioDomainError> {
        let source = self
            .sources
            .iter_mut()
            .find(|s| s.id() == id)
            .ok_or(AudioDomainError("unknown audio source"))?;
        let replacement = AudioSource::new(
            source.id(),
            uri,
            source.content_hash(),
            source.size_bytes(),
            source.metadata(),
        )?;
        *source = replacement;
        Ok(())
    }

    pub fn with_audio(
        id: EntityId,
        name: String,
        sources: Vec<AudioSource>,
        tracks: Vec<VocalTrack>,
    ) -> Result<Self, AudioDomainError> {
        let mut ids = HashSet::from([id]);
        for entity in sources
            .iter()
            .map(AudioSource::id)
            .chain(tracks.iter().map(|t| t.id))
        {
            if !ids.insert(entity) {
                return Err(AudioDomainError("duplicate entity identity"));
            }
        }
        let source_ids: HashSet<_> = sources.iter().map(AudioSource::id).collect();
        if tracks.iter().any(|t| !source_ids.contains(&t.source)) {
            return Err(AudioDomainError("track references missing audio source"));
        }
        Ok(Self {
            id,
            name,
            sources,
            tracks,
        })
    }

    /// Validates the entire addition before mutation; cannot replace existing entities.
    pub fn attach_audio(
        &mut self,
        source: AudioSource,
        track: VocalTrack,
    ) -> Result<(), AudioDomainError> {
        if track.source != source.id()
            || track.id == source.id()
            || [source.id(), track.id].iter().any(|id| {
                *id == self.id
                    || self.sources.iter().any(|s| s.id() == *id)
                    || self.tracks.iter().any(|t| t.id == *id)
            })
        {
            return Err(AudioDomainError("invalid or duplicate audio attachment"));
        }
        self.sources.push(source);
        self.tracks.push(track);
        Ok(())
    }
}
