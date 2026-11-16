use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{AssetName, CatalogEntry, Error, catalog, git, toml_file};

// The repository's layout: `organization/organization.toml` holds the organization's name and
// marks the folder as a federation repository; `organization/knowledge.md` holds what every agent
// of the organization reads; `assets/` is the catalog, with one `<name>.toml` per asset, named
// after it. Every TOML file carries `schema = 1`, and until the first published version a format
// changes in place, with no migration.
const ORGANIZATION: &str = "organization";
const ORGANIZATION_FILE: &str = "organization.toml";
const KNOWLEDGE: &str = "knowledge.md";
const ASSETS: &str = "assets";

/// An organization's federation repository, opened.
#[derive(Debug)]
pub struct Federation {
    root: PathBuf,
    name: String,
}

#[derive(Serialize, Deserialize)]
struct OrganizationFile {
    name: String,
}

impl Federation {
    /// Turns `folder`, which must not exist or be empty, into a git repository holding a new
    /// organization. Nothing is committed: writes stay in the working tree. On failure, what was
    /// made is removed, so creating it again finds the folder as it was.
    pub fn create(folder: &Path, name: &str) -> Result<Self, Error> {
        let existed = match fs::read_dir(folder) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return Err(Error::NotEmpty(folder.to_owned()));
                }
                true
            }
            Err(e) if e.kind() == ErrorKind::NotFound => false,
            Err(e) => return Err(Error::io(folder)(e)),
        };
        let created = init(folder, name);
        if created.is_err() {
            undo_init(folder, existed);
        }
        created
    }

    pub fn open(folder: &Path) -> Result<Self, Error> {
        let path = folder.join(ORGANIZATION).join(ORGANIZATION_FILE);
        let organization: OrganizationFile = match toml_file::read(&path) {
            Err(Error::Io { source, .. }) if source.kind() == ErrorKind::NotFound => {
                return Err(Error::NotFederation(folder.to_owned()));
            }
            read => read?,
        };
        Ok(Self {
            root: folder.to_owned(),
            name: organization.name,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The organization's knowledge, in Markdown.
    pub fn knowledge(&self) -> Result<String, Error> {
        let knowledge = self.root.join(ORGANIZATION).join(KNOWLEDGE);
        fs::read_to_string(&knowledge).map_err(Error::io(&knowledge))
    }

    pub fn register(&self, entry: &CatalogEntry) -> Result<(), Error> {
        catalog::register(&self.root.join(ASSETS), entry)
    }

    /// Every asset in the catalog, ordered by name.
    pub fn assets(&self) -> Result<Vec<CatalogEntry>, Error> {
        catalog::list(&self.root.join(ASSETS))
    }

    pub fn asset(&self, name: &AssetName) -> Result<CatalogEntry, Error> {
        catalog::get(&self.root.join(ASSETS), name)
    }
}

fn init(folder: &Path, name: &str) -> Result<Federation, Error> {
    let dir = folder.join(ORGANIZATION);
    fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
    git::run(folder, &["init"])?;
    let organization = OrganizationFile {
        name: name.to_owned(),
    };
    toml_file::create(&dir.join(ORGANIZATION_FILE), &organization)?;
    let knowledge = dir.join(KNOWLEDGE);
    fs::write(&knowledge, "").map_err(Error::io(&knowledge))?;
    Ok(Federation {
        root: folder.to_owned(),
        name: organization.name,
    })
}

// Best effort: a failure here is dropped, because the error worth returning is the one that made
// `init` fail. Missing parent folders that `init` created stay.
fn undo_init(folder: &Path, existed: bool) {
    if existed {
        let _ = fs::remove_dir_all(folder.join(ORGANIZATION));
        let _ = fs::remove_dir_all(folder.join(".git"));
    } else {
        let _ = fs::remove_dir_all(folder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_refuses_a_folder_with_something_in_it() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("notes.md"), "").unwrap();
        assert!(matches!(
            Federation::create(dir.path(), "Acme"),
            Err(Error::NotEmpty(_))
        ));
    }

    #[test]
    fn create_accepts_an_empty_folder_or_one_that_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        Federation::create(dir.path(), "Acme").unwrap();
        Federation::create(&dir.path().join("new/acme"), "Acme").unwrap();
    }

    #[test]
    fn undo_leaves_the_folder_as_it_was() {
        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("existing");
        fs::create_dir(&existing).unwrap();
        init(&existing, "Acme").unwrap();
        undo_init(&existing, true);
        assert_eq!(fs::read_dir(&existing).unwrap().count(), 0);

        let new = dir.path().join("new");
        init(&new, "Acme").unwrap();
        undo_init(&new, false);
        assert!(!new.exists());
    }

    #[test]
    fn open_refuses_a_folder_without_an_organization() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Federation::open(dir.path()),
            Err(Error::NotFederation(_))
        ));
    }
}
