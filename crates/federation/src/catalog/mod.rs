use std::path::{Path, PathBuf};

use crate::{AssetName, Error, git};

mod entry;
mod folder;

pub(crate) use folder::{get, list, register};

/// What the organization knows of an asset, whether or not it manages it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub name: AssetName,
    /// The person who owns the asset.
    pub owner: String,
    pub location: Location,
    pub stage: Stage,
    /// A managed asset has its own `.federa/`; an external one is only known.
    pub managed: bool,
}

/// Where an asset's code lives. Moving an asset to another repository changes only this.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// The repository: a local path today, a remote URL once repositories are shared.
    pub repo: String,
    /// The asset's folder inside the repository, relative to its root; empty for the root.
    pub folder: PathBuf,
}

impl Location {
    /// Where the code in `folder` lives: the root of the git repository that holds it, and
    /// `folder` relative to that root.
    pub fn find(folder: &Path) -> Result<Self, Error> {
        // git gives both as it sees them, so a symlinked path such as macOS's `/tmp` needs no
        // resolving here. The prefix is empty at the root and ends in `/` below it.
        let found = git::run(folder, &["rev-parse", "--show-toplevel", "--show-prefix"]).map_err(
            |failure| match failure {
                git::Failure::Refused(reason) => Error::NotInRepository {
                    folder: folder.to_owned(),
                    reason,
                },
                not_run => not_run.into(),
            },
        )?;
        let mut lines = found.lines();
        Ok(Self {
            repo: lines.next().unwrap_or_default().to_owned(),
            folder: lines
                .next()
                .unwrap_or_default()
                .trim_end_matches('/')
                .into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Development,
    Production,
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git::run(dir.path(), &["init"]).unwrap();
        dir
    }

    #[test]
    fn finds_the_root_of_the_repository() {
        let dir = repo();
        let root = fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            Location::find(dir.path()).unwrap(),
            Location {
                repo: root.to_string_lossy().into_owned(),
                folder: PathBuf::new(),
            }
        );
    }

    #[test]
    fn finds_a_folder_inside_the_repository() {
        let dir = repo();
        let billing = dir.path().join("services/billing");
        fs::create_dir_all(&billing).unwrap();
        let location = Location::find(&billing).unwrap();
        assert_eq!(location.folder, PathBuf::from("services/billing"));
        assert_eq!(
            Path::new(&location.repo),
            fs::canonicalize(dir.path()).unwrap()
        );
    }

    #[test]
    fn refuses_a_folder_outside_any_repository() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Location::find(dir.path()),
            Err(Error::NotInRepository { .. })
        ));
    }
}
