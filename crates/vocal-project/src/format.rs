use crate::{ProjectError, SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use vocal_domain::{
    audio::{AudioMetadata, AudioSource, AudioUri, ChannelMode, VocalTrack},
    time::Samples,
};
use vocal_domain::{identity::EntityId, project::VocalProject};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Manifest {
    pub format: String,
    pub schema_version: u32,
    pub created_with: String,
    pub project_id: String,
}

impl Manifest {
    pub fn new(project: &VocalProject) -> Self {
        Self {
            format: "vocal-project".into(),
            schema_version: SCHEMA_VERSION,
            created_with: env!("CARGO_PKG_VERSION").into(),
            project_id: project.id().to_string(),
        }
    }

    pub fn validate(&self) -> Result<EntityId, ProjectError> {
        if self.format != "vocal-project" || self.created_with.is_empty() {
            return Err(ProjectError::Invalid(
                "invalid manifest format or createdWith",
            ));
        }
        self.project_id
            .parse()
            .map_err(|_| ProjectError::Invalid("invalid project UUID"))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectV1 {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectV2 {
    pub id: String,
    pub name: String,
    pub sources: Vec<SourceDto>,
    pub tracks: Vec<TrackDto>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SourceDto {
    id: String,
    uri: UriDto,
    content_hash: String,
    size_bytes: u64,
    sample_rate: u32,
    channels: u16,
    frames: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum UriDto {
    Linked {
        #[serde(rename = "absoluteFallback")]
        absolute_fallback: String,
        #[serde(rename = "relativePath")]
        relative_path: Option<String>,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TrackDto {
    id: String,
    name: String,
    source: String,
    channel_mode: ModeDto,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ModeDto {
    EqualPowerMono,
    Left,
    Right,
    Mid,
}

impl From<&VocalProject> for ProjectV2 {
    fn from(project: &VocalProject) -> Self {
        Self {
            id: project.id().to_string(),
            name: project.name().into(),
            sources: project
                .sources()
                .iter()
                .map(|s| {
                    let AudioUri::Linked {
                        absolute_fallback,
                        relative_path,
                    } = s.uri();
                    SourceDto {
                        id: s.id().to_string(),
                        uri: UriDto::Linked {
                            absolute_fallback: absolute_fallback.clone(),
                            relative_path: relative_path.clone(),
                        },
                        content_hash: s.content_hash().to_string(),
                        size_bytes: s.size_bytes(),
                        sample_rate: s.metadata().sample_rate(),
                        channels: s.metadata().channels(),
                        frames: s.metadata().frames().get(),
                    }
                })
                .collect(),
            tracks: project
                .tracks()
                .iter()
                .map(|t| TrackDto {
                    id: t.id.to_string(),
                    name: t.name.clone(),
                    source: t.source.to_string(),
                    channel_mode: match t.channel_mode {
                        ChannelMode::EqualPowerMono => ModeDto::EqualPowerMono,
                        ChannelMode::Left => ModeDto::Left,
                        ChannelMode::Right => ModeDto::Right,
                        ChannelMode::Mid => ModeDto::Mid,
                    },
                })
                .collect(),
        }
    }
}

impl ProjectV2 {
    pub fn into_domain(self, expected: EntityId) -> Result<VocalProject, ProjectError> {
        let id: EntityId = self
            .id
            .parse()
            .map_err(|_| ProjectError::Invalid("invalid project UUID"))?;
        if id != expected {
            return Err(ProjectError::Invalid(
                "manifest and project identities differ",
            ));
        }
        let mut sources = Vec::new();
        for s in self.sources {
            let UriDto::Linked {
                absolute_fallback,
                relative_path,
            } = s.uri;
            let metadata = AudioMetadata::new(s.sample_rate, s.channels, Samples::new(s.frames))
                .map_err(|e| ProjectError::Invalid(e.0))?;
            sources.push(
                AudioSource::new(
                    s.id.parse()
                        .map_err(|_| ProjectError::Invalid("invalid source UUID"))?,
                    AudioUri::Linked {
                        absolute_fallback,
                        relative_path,
                    },
                    s.content_hash
                        .parse()
                        .map_err(|_| ProjectError::Invalid("invalid source hash"))?,
                    s.size_bytes,
                    metadata,
                )
                .map_err(|e| ProjectError::Invalid(e.0))?,
            );
        }
        let mut tracks = Vec::new();
        for t in self.tracks {
            tracks.push(VocalTrack {
                id: t
                    .id
                    .parse()
                    .map_err(|_| ProjectError::Invalid("invalid track UUID"))?,
                name: t.name,
                source: t
                    .source
                    .parse()
                    .map_err(|_| ProjectError::Invalid("invalid source reference"))?,
                channel_mode: match t.channel_mode {
                    ModeDto::EqualPowerMono => ChannelMode::EqualPowerMono,
                    ModeDto::Left => ChannelMode::Left,
                    ModeDto::Right => ChannelMode::Right,
                    ModeDto::Mid => ChannelMode::Mid,
                },
            });
        }
        VocalProject::with_audio(id, self.name, sources, tracks)
            .map_err(|e| ProjectError::Invalid(e.0))
    }
}
