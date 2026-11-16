use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use serde::Serialize;

use crate::token::Token;

// `FEDERA_DATA_DIR` replaces the platform's folder, for tests and for more than one runtime on one
// machine.

/// The runtime's own folder on this machine.
pub(crate) struct DataFolder {
    root: PathBuf,
}

#[derive(Serialize)]
struct Published<'a> {
    schema: u32,
    port: u16,
    token: &'a str,
}

impl DataFolder {
    /// `FEDERA_DATA_DIR` when set, otherwise the app's folder in the platform's data folder;
    /// created if missing.
    pub(crate) fn locate() -> io::Result<Self> {
        let root = match env::var_os("FEDERA_DATA_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => dirs::data_dir()
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        "this platform has no data folder; set FEDERA_DATA_DIR",
                    )
                })?
                .join("federa"),
        };
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// The configuration folder of Claude the app's agents run with, so nothing of the person's own
    /// setup reaches them.
    pub(crate) fn claude_config(&self) -> PathBuf {
        self.root.join("claude")
    }

    /// Where the person's conversations with their peers are kept, private to this runtime:
    /// created if missing, readable and writable only by its owner.
    pub(crate) fn conversations(&self) -> io::Result<PathBuf> {
        let folder = self.root.join("conversations");
        owner_only_folder(&folder)?;
        Ok(folder)
    }

    /// Writes where clients find this runtime, readable and writable only by its owner.
    pub(crate) fn publish(&self, port: u16, token: &Token) -> io::Result<()> {
        let text = toml::to_string(&Published {
            schema: 1,
            port,
            token: token.as_str(),
        })
        .expect("a table of plain values serializes");
        // Written beside it and renamed over it, so a client never reads half of the file, and the
        // file of an earlier run never keeps permissions wider than these.
        let file = self.root.join("runtime.toml");
        let partial = self.root.join("runtime.toml.partial");
        let _ = fs::remove_file(&partial);
        io::Write::write_all(&mut owner_only(&partial)?, text.as_bytes())?;
        fs::rename(&partial, &file)
    }
}

#[cfg(unix)]
fn owner_only(path: &Path) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

// A user's data folder on Windows is already readable only by that user.
#[cfg(not(unix))]
fn owner_only(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

// A folder of an earlier run keeps no permissions wider than these either.
#[cfg(unix)]
fn owner_only_folder(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn owner_only_folder(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn keeps_the_conversations_readable_only_by_their_owner() {
        let dir = tempfile::tempdir().unwrap();
        let data = DataFolder {
            root: dir.path().to_owned(),
        };
        let mode = |folder: &Path| fs::metadata(folder).unwrap().permissions().mode() & 0o777;
        let folder = data.conversations().unwrap();
        assert_eq!(mode(&folder), 0o700);

        fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(mode(&data.conversations().unwrap()), 0o700);
    }
}
