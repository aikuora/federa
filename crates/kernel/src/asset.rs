use std::path::{Path, PathBuf};

use federation::{CatalogEntry, Federation, Location, read_asset};
use protocol::{Asset, AssetName, Failure, Stage};

use crate::failure;

/// Registers the managed asset whose `.federa/` sits in `folder`, owned by `person`, in
/// development: the stage every asset starts in.
pub(crate) fn register(
    federation: &Federation,
    person: &str,
    folder: &Path,
) -> Result<Asset, Failure> {
    let registered = read_asset(folder).and_then(|asset| {
        let entry = CatalogEntry {
            name: asset.name,
            owner: person.to_owned(),
            location: Location::find(folder)?,
            stage: federation::Stage::Development,
            managed: true,
        };
        federation.register(&entry)?;
        Ok(entry)
    });
    registered
        .map(|entry| summary(&entry))
        .map_err(failure::from_federation)
}

pub(crate) fn list(federation: &Federation) -> Result<Vec<Asset>, Failure> {
    let entries = federation.assets().map_err(failure::from_federation)?;
    Ok(entries.iter().map(summary).collect())
}

/// The asset a client names, as the catalog names it.
pub(crate) fn parse(asset: &AssetName) -> Result<federation::AssetName, Failure> {
    federation::AssetName::parse(&asset.0).map_err(failure::from_federation)
}

/// The folder that holds the asset's `.federa/`, where its agents work.
pub(crate) fn folder(entry: &CatalogEntry) -> PathBuf {
    // The location is taken as a local path; a repository elsewhere comes with remote assets.
    Path::new(&entry.location.repo).join(&entry.location.folder)
}

/// The asset as a client sees it.
fn summary(entry: &CatalogEntry) -> Asset {
    Asset {
        name: AssetName(entry.name.to_string()),
        stage: match entry.stage {
            federation::Stage::Development => Stage::Development,
            federation::Stage::Production => Stage::Production,
        },
        managed: entry.managed,
    }
}
