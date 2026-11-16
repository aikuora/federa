use std::{io, path::Path, process::Command};

/// Who uses this runtime: the `user.email` of their global git configuration.
pub(crate) fn identify(folder: &Path) -> io::Result<String> {
    // The global configuration and the files it includes, read from a folder that is no
    // repository, so the identity does not depend on where the runtime starts: an `includeIf
    // "gitdir:"` would otherwise answer for the folder it was launched in. The variables a git hook
    // sets would point git at that hook's repository.
    let output = Command::new("git")
        .args(["config", "--global", "--includes", "--get", "user.email"])
        .current_dir(folder)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .map_err(|e| io::Error::new(e.kind(), format!("could not run git: {e}")))?;
    let email = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if email.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "git has no user.email in its global configuration, the identity this runtime works \
             with; set it with `git config --global user.email you@example.com`",
        ));
    }
    Ok(email)
}
