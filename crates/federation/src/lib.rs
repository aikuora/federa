//! How an organization's federation repository and each asset's `.federa/` folder are stored on
//! disk: their layout, their file formats and their schema numbers. Nothing outside this crate
//! knows a path or a file format inside either of them.

mod asset;
mod catalog;
mod error;
mod name;
mod organization;
mod toml_file;

pub use asset::{AssetFolder, read_asset};
pub use catalog::{CatalogEntry, Location, Stage};
pub use error::Error;
pub use name::AssetName;
pub use organization::Federation;
