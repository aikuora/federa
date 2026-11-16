use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} could not be run; is it installed?")]
    NotInstalled(PathBuf),
    #[error("version {found} is older than the minimum {minimum}")]
    TooOld { found: String, minimum: String },
    #[error("the agent is closed")]
    Closed,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
