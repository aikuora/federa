use std::{fs, path::Path};

use serde::Deserialize;

use crate::{AssetName, Error, toml_file};

// An asset's `.federa/` holds `asset.toml`, with `schema = 1` and the asset's name, so the name
// travels with the asset if it moves to another repository, and `knowledge.md`, with its scope.

/// What a managed asset says about itself in its `.federa/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetFolder {
    pub name: AssetName,
    /// The asset's knowledge, in Markdown.
    pub knowledge: String,
}

#[derive(Deserialize)]
struct AssetFile {
    name: String,
}

/// Reads the `.federa/` inside `folder`, the asset's folder.
pub fn read_asset(folder: &Path) -> Result<AssetFolder, Error> {
    let federa = folder.join(".federa");
    if !federa.is_dir() {
        return Err(Error::NoAssetFolder(folder.to_owned()));
    }
    let path = federa.join("asset.toml");
    let asset: AssetFile = toml_file::read(&path)?;
    let knowledge = federa.join("knowledge.md");
    Ok(AssetFolder {
        name: AssetName::parse(&asset.name).map_err(|e| Error::invalid(&path, e))?,
        knowledge: fs::read_to_string(&knowledge).map_err(Error::io(&knowledge))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_a_federa_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            read_asset(dir.path()),
            Err(Error::NoAssetFolder(_))
        ));
    }

    #[test]
    fn rejects_an_invalid_name() {
        let dir = tempfile::tempdir().unwrap();
        let federa = dir.path().join(".federa");
        fs::create_dir(&federa).unwrap();
        fs::write(
            federa.join("asset.toml"),
            "schema = 1\nname = \"Billing\"\n",
        )
        .unwrap();
        fs::write(federa.join("knowledge.md"), "").unwrap();
        assert!(matches!(read_asset(dir.path()), Err(Error::Invalid { .. })));
    }

    #[test]
    fn names_the_file_it_could_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let federa = dir.path().join(".federa");
        fs::create_dir(&federa).unwrap();
        fs::write(
            federa.join("asset.toml"),
            "schema = 1\nname = \"billing\"\n",
        )
        .unwrap();
        let error = read_asset(dir.path()).unwrap_err();
        assert!(matches!(&error, Error::Io { path, .. } if *path == federa.join("knowledge.md")));
        assert!(error.to_string().contains("knowledge.md"), "{error}");
    }
}
