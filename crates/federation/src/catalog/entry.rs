use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{AssetName, CatalogEntry, Error, Location, Stage, toml_file};

// The entry's file carries no name: the catalog names the file after the asset.
#[derive(Serialize, Deserialize)]
struct EntryFile {
    owner: String,
    location: LocationFile,
    stage: StageFile,
    managed: bool,
}

#[derive(Serialize, Deserialize)]
struct LocationFile {
    repo: String,
    folder: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum StageFile {
    Development,
    Production,
}

pub(crate) fn read(path: &Path, name: AssetName) -> Result<CatalogEntry, Error> {
    let entry: EntryFile = toml_file::read(path)?;
    Ok(CatalogEntry {
        name,
        owner: entry.owner,
        location: Location {
            repo: entry.location.repo,
            folder: entry.location.folder,
        },
        stage: match entry.stage {
            StageFile::Development => Stage::Development,
            StageFile::Production => Stage::Production,
        },
        managed: entry.managed,
    })
}

/// Writes a new entry, and fails with `AlreadyExists` if the file is there.
pub(crate) fn create(path: &Path, entry: &CatalogEntry) -> Result<(), Error> {
    let entry = EntryFile {
        owner: entry.owner.clone(),
        location: LocationFile {
            repo: entry.location.repo.clone(),
            folder: entry.location.folder.clone(),
        },
        stage: match entry.stage {
            Stage::Development => StageFile::Development,
            Stage::Production => StageFile::Production,
        },
        managed: entry.managed,
    };
    toml_file::create(path, &entry)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn name() -> AssetName {
        AssetName::parse("billing").unwrap()
    }

    #[test]
    fn reads_back_what_it_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("billing.toml");
        let entry = CatalogEntry {
            name: name(),
            owner: "ana".to_owned(),
            location: Location {
                repo: "/src/platform".to_owned(),
                folder: "services/billing".into(),
            },
            stage: Stage::Production,
            managed: false,
        };
        create(&path, &entry).unwrap();
        assert_eq!(read(&path, name()).unwrap(), entry);
    }

    #[test]
    fn reads_the_documented_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("billing.toml");
        let text = "schema = 1\nowner = \"ana\"\nstage = \"development\"\nmanaged = true\n\n\
                    [location]\nrepo = \"/src/billing\"\nfolder = \"\"\n";
        fs::write(&path, text).unwrap();
        let entry = read(&path, name()).unwrap();
        assert_eq!(entry.stage, Stage::Development);
        assert_eq!(entry.location.folder, PathBuf::new());
    }

    #[test]
    fn rejects_an_unknown_stage_or_a_missing_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("billing.toml");
        let location = "\n[location]\nrepo = \"/src/billing\"\nfolder = \"\"\n";
        for body in [
            "owner = \"ana\"\nstage = \"staging\"\nmanaged = true\n",
            "owner = \"ana\"\nstage = \"production\"\n",
        ] {
            fs::write(&path, format!("schema = 1\n{body}{location}")).unwrap();
            assert!(
                matches!(read(&path, name()), Err(Error::Invalid { .. })),
                "{body:?}"
            );
        }
        fs::write(
            &path,
            "schema = 1\nowner = \"ana\"\nstage = \"production\"\nmanaged = true\n",
        )
        .unwrap();
        assert!(matches!(read(&path, name()), Err(Error::Invalid { .. })));
    }
}
