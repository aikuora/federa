use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use federation::AssetName;
use protocol::{Failure, FailureCode, Message};
use serde::{Deserialize, Serialize};

// Conversations are the person's own and never leave their runtime: one TOML file per organization
// and asset under the runtime's data folder, with `schema = 1`, every message and the engine's
// session, rewritten in place until the first published version. The organization is told apart by
// its federation repository's folder, the one thing that names it on this machine.

/// The stored conversation with the peer of one asset.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Stored {
    pub(crate) messages: Vec<Message>,
    /// The engine's session, to continue it after the runtime restarts.
    pub(crate) session: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct File {
    schema: u32,
    session: Option<String>,
    #[serde(default)]
    messages: Vec<Message>,
}

const SCHEMA: u32 = 1;

/// Where the person's conversations live.
pub(crate) struct Conversations {
    root: PathBuf,
}

impl Conversations {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The conversation about `asset` in the organization whose repository is `organization`;
    /// empty when there is none yet.
    pub(crate) fn load(&self, organization: &Path, asset: &AssetName) -> Result<Stored, Failure> {
        let path = self.file(organization, asset)?;
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Stored::default()),
            Err(error) => return Err(internal(&path, error)),
        };
        let file: File = toml::from_str(&text).map_err(|error| internal(&path, error))?;
        if file.schema != SCHEMA {
            let reason = format!("schema {} is not one this version reads", file.schema);
            return Err(internal(&path, reason));
        }
        Ok(Stored {
            messages: file.messages,
            session: file.session,
        })
    }

    pub(crate) fn save(
        &self,
        organization: &Path,
        asset: &AssetName,
        stored: &Stored,
    ) -> Result<(), Failure> {
        let path = self.file(organization, asset)?;
        let file = File {
            schema: SCHEMA,
            session: stored.session.clone(),
            messages: stored.messages.clone(),
        };
        let text = toml::to_string(&file).map_err(|error| internal(&path, error))?;
        write(&path, &text).map_err(|error| internal(&path, error))
    }

    fn file(&self, organization: &Path, asset: &AssetName) -> Result<PathBuf, Failure> {
        let folder = fs::canonicalize(organization).map_err(|e| internal(organization, e))?;
        Ok(self
            .root
            .join(key(&folder))
            .join(format!("{}.toml", asset.as_str())))
    }
}

// Written beside it and renamed over it, so a runtime that stops halfway never leaves half a
// conversation.
fn write(path: &Path, text: &str) -> io::Result<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let partial = path.with_extension("toml.partial");
    fs::write(&partial, text)?;
    fs::rename(&partial, path)
}

// FNV-1a by hand rather than std's `DefaultHasher`, whose output may change with any Rust release
// and would then lose every conversation; a stable hash keeps the key the same across toolchains.
// Its 64 bits make two organizations of one machine colliding a risk accepted as negligible.
/// The folder of an organization's conversations, from its repository's canonical folder.
fn key(folder: &Path) -> String {
    let hash = folder
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
        });
    format!("{hash:016x}")
}

fn internal(path: &Path, reason: impl std::fmt::Display) -> Failure {
    Failure {
        code: FailureCode::Internal,
        message: format!("{}: {reason}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use protocol::Author;

    use super::*;

    fn billing() -> AssetName {
        AssetName::parse("billing").unwrap()
    }

    #[test]
    fn keeps_every_message_and_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let acme = dir.path().join("acme");
        fs::create_dir(&acme).unwrap();
        let conversations = Conversations::new(dir.path().join("conversations"));
        assert_eq!(
            conversations.load(&acme, &billing()).unwrap(),
            Stored::default()
        );

        let stored = Stored {
            messages: vec![
                Message {
                    author: Author::Person,
                    text: "What does it do?\n\"quoted\"".to_owned(),
                },
                Message {
                    author: Author::Peer,
                    text: "It charges customers.".to_owned(),
                },
            ],
            session: Some("s-1".to_owned()),
        };
        conversations.save(&acme, &billing(), &stored).unwrap();
        conversations.save(&acme, &billing(), &stored).unwrap();
        assert_eq!(conversations.load(&acme, &billing()).unwrap(), stored);
        // Spelled another way, the folder is still the same organization.
        let spelled = dir.path().join("acme/../acme");
        assert_eq!(conversations.load(&spelled, &billing()).unwrap(), stored);
        let ledger = AssetName::parse("ledger").unwrap();
        assert_eq!(
            conversations.load(&acme, &ledger).unwrap(),
            Stored::default()
        );
        let globex = dir.path().join("globex");
        fs::create_dir(&globex).unwrap();
        assert_eq!(
            conversations.load(&globex, &billing()).unwrap(),
            Stored::default()
        );
    }

    #[test]
    fn writes_the_schema_and_refuses_another() {
        let dir = tempfile::tempdir().unwrap();
        let conversations = Conversations::new(dir.path().join("conversations"));
        conversations
            .save(dir.path(), &billing(), &Stored::default())
            .unwrap();
        let path = conversations.file(dir.path(), &billing()).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("schema = 1\n"), "{text}");

        fs::write(&path, "schema = 2\n").unwrap();
        let failure = conversations.load(dir.path(), &billing()).unwrap_err();
        assert_eq!(failure.code, FailureCode::Internal);
        assert!(failure.message.contains("schema 2"), "{}", failure.message);
    }

    #[test]
    fn the_key_of_a_folder_never_changes() {
        // Pinned to what the reference FNV-1a 64 gives, so a change that would lose every
        // conversation fails here.
        assert_eq!(key(Path::new("/work/acme")), "3a4b27f3e9add09a");
    }
}
