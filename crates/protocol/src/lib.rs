//! How everything between a client and the runtime looks on the wire: internally tagged JSON
//! frames that open with a `Hello` declaring the version, exactly one reply per command keyed by
//! the request id the client chose, and the peer's answer as events of its turn. The TypeScript
//! types of a client are generated from these.

mod asset;
mod conversation;
mod failure;
mod frame;
mod organization;
mod sign_in;

pub use asset::{Asset, AssetName, Stage};
pub use conversation::{Author, Conversation, Message, TurnId};
pub use failure::{Failure, FailureCode};
pub use frame::{ClientFrame, Command, Event, Hello, Outcome, Reply, RequestId, RuntimeFrame};
pub use organization::Organization;
pub use sign_in::SignInStatus;

/// The version of this contract. Both sides declare it in their `Hello`.
pub const VERSION: u32 = 1;
