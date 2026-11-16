use std::fmt::Write;

use federation::AssetName;

// Each agent gets a token of its own when it opens, and every tool call it makes carries it, so
// the kernel knows who asks without trusting anything the agent says about itself.

/// Who an agent is: what the kernel checks on every request the agent makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentIdentity {
    pub(crate) role: Role,
    /// The asset the agent works on.
    pub(crate) asset: AssetName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    Peer,
}

/// The secret an agent presents on every tool call. It never reaches a log.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct AgentToken(String);

impl AgentToken {
    /// A new random token, drawn from the operating system.
    pub(crate) fn generate() -> Self {
        let mut bytes = [0u8; 32];
        // Without the operating system's randomness there is no token worth admitting an agent with.
        getrandom::fill(&mut bytes).expect("the operating system provides random bytes");
        Self(bytes.iter().fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        }))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for AgentToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AgentToken(..)")
    }
}
