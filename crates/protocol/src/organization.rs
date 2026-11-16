use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The organization the runtime has open, and the folder of its federation repository.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Organization {
    pub name: String,
    pub folder: PathBuf,
}
