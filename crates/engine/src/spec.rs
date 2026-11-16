use std::path::PathBuf;

// The agent's role reaches the engine as part of its instructions, and as the identity behind its
// MCP token; the engine needs nothing else from it, so no role type crosses this contract.

/// What an agent is opened with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    /// Who the agent is and what it must follow, appended to the engine's own instructions.
    pub instructions: String,
    /// The folder the agent works in, and the only one its sandbox lets it write.
    pub workdir: PathBuf,
    /// The MCP servers the agent can call, each one's tools allowed without asking.
    pub mcp: Vec<McpServer>,
}

/// An MCP server reached over HTTP, with the bearer token that identifies the agent to it.
#[derive(Clone, PartialEq, Eq)]
pub struct McpServer {
    pub name: String,
    pub url: String,
    pub token: String,
}

impl std::fmt::Debug for McpServer {
    // The token never reaches a log.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpServer")
            .field("name", &self.name)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}
