use std::path::PathBuf;

use crate::AssetName;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Development,
    Production,
}
