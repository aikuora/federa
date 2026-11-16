use std::io;
use std::path::Path;

// The configuration folder is the agents' own, so this crate owns its `settings.json` and rewrites
// it whole; the rest of the folder, such as the sign-in, belongs to the binary. The file is the
// documented scope of these keys, which `--settings` is not.
//
// Nothing synced from the person's claude.ai account loads, and the built-in plugin that reads the
// workdir's AGENTS.md reads only CLAUDE.md, which leaving out the project source already cuts. The
// binary always opens a socket for messages from the person's other sessions; every one is dropped.
const SETTINGS: &str = r#"{
  "crossSessionInbound": "refuse",
  "syncClaudeAiPlugins": false,
  "syncClaudeAiSkills": false,
  "pluginConfigs": {
    "cc-plugin-agents-md@builtin": { "options": { "instructionFiles": "claude-md" } }
  }
}
"#;

/// Makes `folder` ready for agents to run on.
pub(crate) fn prepare(folder: &Path) -> io::Result<()> {
    std::fs::create_dir_all(folder)?;
    std::fs::write(folder.join("settings.json"), SETTINGS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_settings_that_turn_off_what_the_person_and_the_workdir_bring() {
        let folder = std::env::temp_dir().join(format!("claude-config-{}", std::process::id()));
        prepare(&folder).unwrap();
        prepare(&folder).unwrap();
        let text = std::fs::read_to_string(folder.join("settings.json")).unwrap();
        std::fs::remove_dir_all(&folder).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(settings["crossSessionInbound"], "refuse");
        assert_eq!(settings["syncClaudeAiPlugins"], false);
        assert_eq!(settings["syncClaudeAiSkills"], false);
        let agents_md = &settings["pluginConfigs"]["cc-plugin-agents-md@builtin"];
        assert_eq!(agents_md["options"]["instructionFiles"], "claude-md");
    }
}
