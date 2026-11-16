//! What the kernel's tests share: driving it by command, and an asset repository to register.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as Process,
};

use kernel::Kernel;
use protocol::{Command, Failure, Outcome, Reply};

pub async fn done(kernel: &Kernel, command: Command) -> Reply {
    match kernel.handle(command).await {
        Outcome::Done { reply } => reply,
        Outcome::Failed { failure } => panic!("{failure:?}"),
    }
}

pub async fn failed(kernel: &Kernel, command: Command) -> Failure {
    match kernel.handle(command).await {
        Outcome::Failed { failure } => failure,
        Outcome::Done { reply } => panic!("{reply:?}"),
    }
}

/// A git repository holding the asset `billing` in `services/billing`, as in a monorepo.
pub fn platform(dir: &Path) -> PathBuf {
    let repo = dir.join("platform");
    let federa = repo.join("services/billing/.federa");
    fs::create_dir_all(&federa).unwrap();
    fs::write(
        federa.join("asset.toml"),
        "schema = 1\nname = \"billing\"\n",
    )
    .unwrap();
    fs::write(federa.join("knowledge.md"), "# billing\n").unwrap();
    // A git hook sets these for the repository it runs in, and git would init that one instead.
    let init = Process::new("git")
        .arg("init")
        .arg(&repo)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(init.status.success());
    repo
}
