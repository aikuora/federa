use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An asset's name in its organization's catalog. The runtime validates it; clients only carry it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct AssetName(pub String);

/// An asset as a client shows it in the map.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Asset {
    pub name: AssetName,
    pub stage: Stage,
    pub managed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Development,
    Production,
}
