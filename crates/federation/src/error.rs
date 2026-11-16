use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use crate::AssetName;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} is not an empty folder")]
    NotEmpty(PathBuf),
    #[error("{0} is not a federation repository")]
    NotFederation(PathBuf),
    #[error("{0} has no .federa/ folder")]
    NoAssetFolder(PathBuf),
    #[error("asset {0} is already registered")]
    AlreadyRegistered(AssetName),
    #[error("asset {0} is not registered")]
    NotRegistered(AssetName),
    #[error("{0:?} is not a valid asset name: lowercase letters, digits and single hyphens")]
    InvalidName(String),
    /// A file whose name or content this version cannot read or write, such as one in another
    /// schema.
    #[error("{file}: {reason}")]
    Invalid { file: PathBuf, reason: String },
    #[error("git failed: {0}")]
    Git(String),
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

impl Error {
    pub(crate) fn invalid(file: &Path, reason: impl fmt::Display) -> Self {
        Self::Invalid {
            file: file.to_owned(),
            reason: reason.to_string(),
        }
    }

    /// What `map_err` takes for an I/O error on `path`.
    pub(crate) fn io(path: &Path) -> impl FnOnce(io::Error) -> Self {
        move |source| Self::Io {
            path: path.to_owned(),
            source,
        }
    }
}
