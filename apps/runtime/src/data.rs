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
