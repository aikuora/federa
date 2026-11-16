use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Why a command or a turn failed: a code a client can act on, and a message the person can read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct Failure {
    pub code: FailureCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum FailureCode {
    /// The command carries a value the runtime refuses, such as a folder that is not empty.
    Invalid,
    NotFound,
    AlreadyExists,
    /// The command needs an open organization and there is none.
    NoOrganization,
    /// The person has not signed in to the agent engine; the message says how.
    NotSignedIn,
    Internal,
}
