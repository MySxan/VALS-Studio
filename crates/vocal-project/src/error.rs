use std::{error::Error, fmt, io};

#[derive(Debug)]
pub enum ProjectError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
    Invalid(&'static str),
    UnsupportedVersion(u32),
    Migration(String),
}

impl ProjectError {
    pub fn message_key(&self) -> &'static str {
        match self {
            Self::Io(_) => "project.io_failed",
            Self::UnsupportedVersion(_) => "project.unsupported_schema",
            Self::Migration(_) => "project.migration_failed",
            _ => "project.invalid",
        }
    }
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {self:?}", self.message_key())
    }
}

impl Error for ProjectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Zip(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ProjectError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<zip::result::ZipError> for ProjectError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e)
    }
}
impl From<serde_json::Error> for ProjectError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
