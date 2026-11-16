use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whether the agents' engine is ready: signed in, signed out, signing in while the person
/// completes the provider's sign-in in the browser, or unavailable, with the reason, when the
/// runtime cannot run it at all, such as when it is not installed. A sign-in that does not
/// complete in time goes back to `signed_out`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SignInStatus {
    SignedIn,
    SignedOut,
    SigningIn,
    Unavailable { reason: String },
}
