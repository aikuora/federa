//! What of the runtime's environment reaches the binary. Alone in its own test binary, because it
//! sets a variable of the process, which no other thread may read meanwhile.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use claude::Claude;
use engine::{AgentSpec, Engine, Event, SignIn};

#[tokio::test]
async fn passes_the_binary_none_of_the_runtimes_secrets() {
    let folder = std::env::temp_dir().join(format!("claude-environment-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let binary = folder.join("claude");
    // Answers with what it sees of three variables: a secret, its own folder and its home. Like the
    // real binary with a key in its environment, it says it is signed in when it sees the secret.
    let script = r#"#!/bin/sh
if [ "$1" = --version ]; then echo "2.1.311 (Claude Code)"; exit 0; fi
if [ "$1 $2" = "auth status" ]; then test -n "$RUNTIME_SECRET"; exit; fi
read -r line
seen="${RUNTIME_SECRET:-none}|$CLAUDE_CONFIG_DIR|${HOME:+home}"
echo '{"type":"system","subtype":"init","session_id":"s1"}'
echo "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"$seen\"}]}}"
echo '{"type":"result","subtype":"success","is_error":false}'
"#;
    std::fs::write(&binary, script).unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    // SAFETY: this test binary runs this one test, so no other thread reads the environment.
    unsafe { std::env::set_var("RUNTIME_SECRET", "sk-not-for-agents") };
    let config = folder.join("config");
    let claude = Claude::new(binary, config.clone()).unwrap();
    assert_eq!(claude.sign_in_status().unwrap(), SignIn::SignedOut);
    let spec = AgentSpec {
        instructions: "You are a test agent.".into(),
        workdir: folder.clone(),
        mcp: vec![],
        resume: None,
    };
    let mut agent = claude.open(spec).unwrap();
    agent.send("what do you see?".into()).await.unwrap();
    let mut seen = None;
    while let Some(event) = agent.next().await {
        match event {
            Event::Text(text) => seen = Some(text),
            Event::TurnEnded => break,
            _ => {}
        }
    }
    agent.close().await;
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(seen, Some(format!("none|{}|home", config.display())));
}
