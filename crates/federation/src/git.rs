use std::{path::Path, process::Command};

use crate::Error;

/// What a git command run in `dir` failed with.
#[derive(Debug)]
pub(crate) enum Failure {
    /// git could not be run at all.
    NotRun(String),
    /// git ran and refused, with what it wrote to stderr.
    Refused(String),
}

impl From<Failure> for Error {
    fn from(failure: Failure) -> Self {
        match failure {
            Failure::NotRun(reason) | Failure::Refused(reason) => Self::Git(reason),
        }
    }
}

/// Runs git with `args` in `dir` and returns its stdout.
pub(crate) fn run(dir: &Path, args: &[&str]) -> Result<String, Failure> {
    // git reads these from the environment before any argument, and a git hook sets them for the
    // repository it runs in; inherited, git would act on that repository instead of `dir`.
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .map_err(|e| Failure::NotRun(format!("could not run git: {e}")))?;
    if !output.status.success() {
        return Err(Failure::Refused(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
