use federation::Federation;
use serde_json::{Value, json};

use crate::agent::{AgentIdentity, Role};

mod knowledge;

/// A tool an agent can call, as an MCP server describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    /// The JSON Schema of its input.
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    /// The token belongs to no open agent.
    #[error("unknown agent")]
    Unauthorized,
    #[error("there is no tool {0}")]
    UnknownTool(String),
    /// The input does not match the tool's schema.
    #[error("invalid input: {0}")]
    Invalid(String),
    /// The tool ran and failed; the message is for the agent to read.
    #[error("{0}")]
    Failed(String),
}

/// The tools an agent of this identity may call.
pub(crate) fn available(agent: &AgentIdentity) -> Vec<Tool> {
    match agent.role {
        Role::Peer => vec![Tool {
            name: ASSET_KNOWLEDGE,
            description: "Returns the knowledge of the asset you work on: its scope, its \
                          contracts and how it is built, tested and run.",
            input_schema: json!({ "type": "object", "properties": {} }),
        }],
    }
}

const ASSET_KNOWLEDGE: &str = "asset_knowledge";

/// Runs `tool` for `agent`, if its identity may call it.
pub(crate) fn run(
    federation: &Federation,
    agent: &AgentIdentity,
    tool: &str,
    input: &Value,
) -> Result<String, ToolError> {
    if !available(agent).iter().any(|offered| offered.name == tool) {
        return Err(ToolError::UnknownTool(tool.to_owned()));
    }
    if !input.is_object() {
        return Err(ToolError::Invalid("the input is not an object".to_owned()));
    }
    match tool {
        ASSET_KNOWLEDGE => knowledge::asset_knowledge(federation, agent),
        other => Err(ToolError::UnknownTool(other.to_owned())),
    }
}
