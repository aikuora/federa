use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use crate::{AssetName, CatalogEntry, Error, catalog::entry};

// The catalog is a folder with one `<name>.toml` per asset. The folder is created by the first
// registration and a missing one is an empty catalog: git keeps no empty folder, so a clone of a
// repository with no assets arrives without it.

pub(crate) fn register(dir: &Path, entry: &CatalogEntry) -> Result<(), Error> {
    fs::create_dir_all(dir).map_err(Error::io(dir))?;
    match entry::create(&path(dir, &entry.name), entry) {
        Err(Error::Io { source, .. }) if source.kind() == ErrorKind::AlreadyExists => {
            Err(Error::AlreadyRegistered(entry.name.clone()))
        }
        created => created,
    }
}

/// Every asset in the catalog, ordered by name.
pub(crate) fn list(dir: &Path) -> Result<Vec<CatalogEntry>, Error> {
    let files = match fs::read_dir(dir) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        files => files.map_err(Error::io(dir))?,
    };
    let mut assets = Vec::new();
    for file in files {
        let path = file.map_err(Error::io(dir))?.path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            return Err(Error::invalid(&path, "its name is not UTF-8"));
        };
        // A hidden file is not an entry, such as the `._<name>.toml` macOS writes on some disks.
        if stem.starts_with('.') {
            continue;
        }
        let name = AssetName::parse(stem).map_err(|e| Error::invalid(&path, e))?;
        assets.push(entry::read(&path, name)?);
    }
    assets.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(assets)
}

pub(crate) fn get(dir: &Path, name: &AssetName) -> Result<CatalogEntry, Error> {
    match entry::read(&path(dir, name), name.clone()) {
        Err(Error::Io { source, .. }) if source.kind() == ErrorKind::NotFound => {
            Err(Error::NotRegistered(name.clone()))
        }
        read => read,
    }
}

fn path(dir: &Path, name: &AssetName) -> PathBuf {
    dir.join(format!("{name}.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Location, Stage};

    fn entry(name: &str) -> CatalogEntry {
        CatalogEntry {
            name: AssetName::parse(name).unwrap(),
            owner: "ana".to_owned(),
            location: Location {
                repo: format!("/src/{name}"),
                folder: PathBuf::new(),
            },
            stage: Stage::Development,
            managed: true,
        }
    }

    #[test]
    fn a_missing_folder_is_an_empty_catalog() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(list(&dir.path().join("assets")).unwrap(), []);
    }

    #[test]
    fn lists_entries_ordered_by_name_and_skips_other_files() {
        let dir = tempfile::tempdir().unwrap();
        register(dir.path(), &entry("web")).unwrap();
        register(dir.path(), &entry("api")).unwrap();
        fs::write(dir.path().join("._api.toml"), [0, 5, 22, 7]).unwrap();
        fs::write(dir.path().join("notes.md"), "").unwrap();
        assert_eq!(list(dir.path()).unwrap(), [entry("api"), entry("web")]);
    }

    #[test]
    fn a_file_must_be_named_after_a_valid_asset() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Billing.toml"), "").unwrap();
        assert!(matches!(list(dir.path()), Err(Error::Invalid { .. })));
    }

    #[test]
    fn registers_an_asset_once() {
        let dir = tempfile::tempdir().unwrap();
        register(dir.path(), &entry("api")).unwrap();
        assert!(matches!(
            register(dir.path(), &entry("api")),
            Err(Error::AlreadyRegistered(_))
        ));
    }

    #[test]
    fn get_reports_an_asset_that_is_not_registered() {
        let dir = tempfile::tempdir().unwrap();
        let name = AssetName::parse("billing").unwrap();
        assert!(matches!(
            get(dir.path(), &name),
            Err(Error::NotRegistered(_))
        ));
    }
}
