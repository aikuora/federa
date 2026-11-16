use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AssetName;

/// One answer of the peer, from the message that asked for it to its last part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct TurnId(pub u32);

/// The person's conversation with the peer of an asset, oldest message first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Conversation {
    pub asset: AssetName,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Message {
    pub author: Author,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Author {
    Person,
    Peer,
}
