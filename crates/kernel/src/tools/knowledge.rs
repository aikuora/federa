use federation::{Federation, read_asset};

use crate::{AgentIdentity, ToolError, asset};

/// The knowledge of the agent's own asset, as its `.federa/knowledge.md` holds it.
pub(crate) fn asset_knowledge(
    federation: &Federation,
    agent: &AgentIdentity,
) -> Result<String, ToolError> {
    federation
        .asset(&agent.asset)
        .and_then(|entry| read_asset(&asset::folder(&entry)))
        .map(|asset| asset.knowledge)
        .map_err(|error| ToolError::Failed(error.to_string()))
}
