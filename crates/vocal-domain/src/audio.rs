use crate::{
    identity::EntityId,
    time::{Samples, Seconds},
};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDomainError(pub &'static str);
impl fmt::Display for AudioDomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for AudioDomainError {}

/// SHA-256 digest. AudioSource uses original file bytes; analysis keys/artifacts
/// use their own versioned encodings, never a filename or timestamp as identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentHash(pub [u8; 32]);
impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
impl FromStr for ContentHash {
    type Err = AudioDomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AudioDomainError(
                "expected 64 hexadecimal SHA-256 characters",
            ));
        }
        let mut bytes = [0; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)
                .map_err(|_| AudioDomainError("invalid hash"))?;
        }
        Ok(Self(bytes))
    }
}

/// Paths are opaque strings in the domain so foreign-platform projects can open.
/// Filesystem adapters validate/resolve them on the current host before use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioUri {
    Linked {
        absolute_fallback: String,
        relative_path: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioMetadata {
    sample_rate: u32,
    channels: u16,
    frames: Samples,
}
impl AudioMetadata {
    pub fn new(sample_rate: u32, channels: u16, frames: Samples) -> Result<Self, AudioDomainError> {
        if sample_rate == 0 || channels == 0 || frames.get() <= 0 {
            return Err(AudioDomainError(
                "audio requires positive rate, channels and frame count",
            ));
        }
        Ok(Self {
            sample_rate,
            channels,
            frames,
        })
    }
    pub fn sample_rate(self) -> u32 {
        self.sample_rate
    }
    pub fn channels(self) -> u16 {
        self.channels
    }
    pub fn frames(self) -> Samples {
        self.frames
    }
    pub fn duration(self) -> Seconds {
        Seconds::new(self.frames.get() as f64 / self.sample_rate as f64)
            .expect("validated finite duration")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioSource {
    id: EntityId,
    uri: AudioUri,
    content_hash: ContentHash,
    size_bytes: u64,
    metadata: AudioMetadata,
}
impl AudioSource {
    pub fn new(
        id: EntityId,
        uri: AudioUri,
        content_hash: ContentHash,
        size_bytes: u64,
        metadata: AudioMetadata,
    ) -> Result<Self, AudioDomainError> {
        let AudioUri::Linked {
            absolute_fallback,
            relative_path,
        } = &uri;
        if size_bytes == 0
            || absolute_fallback.is_empty()
            || absolute_fallback.contains('\0')
            || relative_path.as_ref().is_some_and(|p| {
                p.is_empty()
                    || p.contains('\0')
                    || p.starts_with(['/', '\\'])
                    || p.contains(':')
                    || p.split(['/', '\\']).any(|part| part == "..")
            })
        {
            return Err(AudioDomainError(
                "invalid linked source location or byte size",
            ));
        }
        Ok(Self {
            id,
            uri,
            content_hash,
            size_bytes,
            metadata,
        })
    }
    pub fn id(&self) -> EntityId {
        self.id
    }
    pub fn uri(&self) -> &AudioUri {
        &self.uri
    }
    pub fn content_hash(&self) -> ContentHash {
        self.content_hash
    }
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
    pub fn metadata(&self) -> AudioMetadata {
        self.metadata
    }
}

/// Selection intent only; importing never modifies or mixes the source samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    EqualPowerMono,
    Left,
    Right,
    Mid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VocalTrack {
    pub id: EntityId,
    pub name: String,
    pub source: EntityId,
    pub channel_mode: ChannelMode,
}
