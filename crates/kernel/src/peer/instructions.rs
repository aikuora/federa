use federation::AssetName;

// The peer reads in full what it must follow: its role, the organization's knowledge and its own
// asset's knowledge are written into its instructions, never left to the model choosing to call a
// tool, since a rule that must be followed cannot depend on a search finding it. The
// `asset_knowledge` tool is offered besides, for reading it again.

/// What the peer of `asset` is told before the person's first message.
pub(crate) fn peer(asset: &AssetName, organization: &str, asset_knowledge: &str) -> String {
    format!(
        "You are the peer of the asset `{asset}`: the agent the person talks with about it. You \
         understand it technically and as a product, advise the person, discuss with them and \
         answer with evidence from its code. You work in the asset's folder, which you can read \
         but not change.\n\
         \n\
         Below is the knowledge you must follow, in full: the organization's, then the asset's. \
         The `asset_knowledge` tool returns the asset's knowledge as it stands now, if you need \
         to read it again.\n\
         \n\
         # The organization's knowledge\n\
         \n\
         {organization}\n\
         \n\
         # The knowledge of `{asset}`\n\
         \n\
         {asset_knowledge}\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carries_the_role_and_both_knowledges_whole() {
        let billing = AssetName::parse("billing").unwrap();
        let instructions = peer(&billing, "# Acme\n\nShip weekly.", "# billing\n\nCharges.");
        assert!(instructions.starts_with("You are the peer of the asset `billing`"));
        assert!(instructions.contains("\n# Acme\n\nShip weekly.\n"));
        assert!(instructions.contains("\n# billing\n\nCharges.\n"));
        assert!(instructions.contains("`asset_knowledge`"));
    }
}
