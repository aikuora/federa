//! Two turns with the `claude` installed on this machine, on the person's own subscription. Only
//! the person runs it, once `recordings/record.ts` has signed in its configuration folder:
//! `cargo test -p claude --test live -- --ignored`.

use std::path::PathBuf;
use std::process::Command;

use claude::Claude;
use engine::{Agent, AgentSpec, Engine, Event};

// The folder the recording script signs in, in the main checkout's `target/` whatever worktree
// runs the test.
fn signed_in_config() -> PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    let git = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
    git.parent().unwrap().join("target/claude-config")
}

async fn turn(agent: &mut Agent, message: &str) -> String {
    agent.send(message.into()).await.unwrap();
    let mut answer = String::new();
    while let Some(event) = agent.next().await {
        match event {
            Event::Text(text) => answer.push_str(&text),
            Event::TurnEnded => return answer,
            Event::Failed(failure) => panic!("the turn failed: {failure}"),
            Event::Started { .. } | Event::ToolCalled { .. } => {}
        }
    }
    panic!("the agent ended in the middle of a turn");
}

#[tokio::test]
#[ignore = "runs the installed claude on the person's subscription"]
async fn converses_two_turns_with_the_installed_binary() {
    let claude = Claude::new(PathBuf::from("claude"), signed_in_config()).unwrap();
    let workdir = std::env::temp_dir().join(format!("claude-live-{}", std::process::id()));
    std::fs::create_dir_all(&workdir).unwrap();
    let spec = AgentSpec {
        instructions: "You are a test agent. Answer as briefly as you are asked to.".into(),
        workdir: workdir.clone(),
        mcp: vec![],
    };
    let mut agent = claude.open(spec).unwrap();
    turn(&mut agent, "Remember the word orange. Reply only: ok").await;
    let answer = turn(
        &mut agent,
        "Which word did I ask you to remember? Reply with it only.",
    )
    .await;
    agent.close().await;
    std::fs::remove_dir_all(workdir).unwrap();
    assert!(answer.to_lowercase().contains("orange"), "{answer}");
}
