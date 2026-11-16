//! The engine against the fake `claude` in `fake/`, built from the real recordings: signing in,
//! conversing and calling a tool of an MCP server over HTTP.
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::post};
use claude::Claude;
use engine::{Agent, AgentSpec, Engine, Event, Failure, McpServer, SignIn};
use serde_json::{Value, json};
use tempfile::TempDir;

fn fake() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fake/claude")
}

/// Runs `claude auth <action>` on `config` and says whether it succeeded.
fn auth(config: &Path, action: &str) -> bool {
    let mut command = Command::new(fake());
    command
        .args(["auth", action])
        .env("CLAUDE_CONFIG_DIR", config);
    command.status().unwrap().success()
}

// The configuration folder lives inside `folder`, which is also the agent's workdir.
fn claude(folder: &TempDir, signed_in: bool) -> Claude {
    let config = folder.path().join("config");
    if signed_in {
        assert!(auth(&config, "login"));
    }
    Claude::new(fake(), config).unwrap()
}

fn spec(folder: &TempDir, instructions: &str, mcp: Vec<McpServer>) -> AgentSpec {
    AgentSpec {
        instructions: instructions.into(),
        workdir: folder.path().to_owned(),
        mcp,
    }
}

// Every event of one turn, from after the message to its end.
async fn turn(agent: &mut Agent, message: &str) -> Vec<Event> {
    agent.send(message.into()).await.unwrap();
    let mut events = Vec::new();
    while let Some(event) = agent.next().await {
        match event {
            Event::Started { .. } => {}
            Event::TurnEnded => return events,
            other => events.push(other),
        }
    }
    panic!("the agent ended in the middle of a turn: {events:?}");
}

#[tokio::test]
async fn answers_each_turn_with_the_message_it_was_sent() {
    let folder = TempDir::new().unwrap();
    let mut agent = claude(&folder, true)
        .open(spec(&folder, "", vec![]))
        .unwrap();
    for message in ["one", "two"] {
        assert_eq!(
            turn(&mut agent, message).await,
            [Event::Text(message.into())]
        );
    }
    agent.close().await;
}

#[tokio::test]
async fn tells_to_sign_in_until_its_folder_is_signed_in() {
    let folder = TempDir::new().unwrap();
    let config = folder.path().join("config");
    let mut agent = claude(&folder, false)
        .open(spec(&folder, "", vec![]))
        .unwrap();
    let events = turn(&mut agent, "ready?").await;
    assert!(
        matches!(events[..], [Event::Failed(Failure::NotSignedIn { .. })]),
        "{events:?}"
    );
    assert!(!auth(&config, "status"));
    assert!(auth(&config, "login"));
    assert!(auth(&config, "status"));
    assert_eq!(
        turn(&mut agent, "ready?").await,
        [Event::Text("ready?".into())]
    );
    agent.close().await;
}

#[test]
fn signs_its_folder_in_through_the_official_sign_in() {
    let folder = TempDir::new().unwrap();
    let claude = claude(&folder, false);
    assert_eq!(claude.sign_in_status().unwrap(), SignIn::SignedOut);
    claude.start_sign_in().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while claude.sign_in_status().unwrap() != SignIn::SignedIn {
        assert!(Instant::now() < deadline, "the sign-in never completed");
        std::thread::sleep(Duration::from_millis(50));
    }
    claude.stop_sign_in();
}

const TOKEN: &str = "agent-token";

// An MCP server with one tool, `secret_word`, that answers only requests carrying `TOKEN`.
async fn mcp_server() -> String {
    async fn answer(headers: HeaderMap, Json(request): Json<Value>) -> Response {
        let authorization = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        if authorization != Some(&format!("Bearer {TOKEN}")) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        // A notification, which gets no answer.
        if request.get("id").is_none() {
            return StatusCode::ACCEPTED.into_response();
        }
        let result = match request["method"].as_str() {
            Some("initialize") => json!({
                "protocolVersion": request["params"]["protocolVersion"],
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "fixture", "version": "1.0.0" },
            }),
            Some("tools/list") => json!({ "tools": [{
                "name": "secret_word",
                "description": "Returns the secret word.",
                "inputSchema": { "type": "object", "properties": {} },
            }] }),
            Some("tools/call") => json!({ "content": [{ "type": "text", "text": "harbor" }] }),
            other => panic!("the fake asked for {other:?}"),
        };
        Json(json!({ "jsonrpc": "2.0", "id": request["id"], "result": result })).into_response()
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/mcp", listener.local_addr().unwrap());
    let router = Router::new().route("/mcp", post(answer));
    tokio::spawn(async move { axum::serve(listener, router).await });
    url
}

fn server(url: String, token: &str) -> Vec<McpServer> {
    vec![McpServer {
        name: "fixture".into(),
        url,
        token: token.into(),
    }]
}

#[tokio::test]
async fn calls_the_tool_its_instructions_name_and_answers_with_its_text() {
    let folder = TempDir::new().unwrap();
    let instructions = "Read secret_word before you answer.";
    let spec = spec(&folder, instructions, server(mcp_server().await, TOKEN));
    let mut agent = claude(&folder, true).open(spec).unwrap();
    let called = Event::ToolCalled {
        name: "mcp__fixture__secret_word".into(),
    };
    let expected = [called, Event::Text("harbor".into())];
    assert_eq!(turn(&mut agent, "What is the word?").await, expected);
    agent.close().await;
}

#[tokio::test]
async fn calls_the_tool_the_message_names() {
    let folder = TempDir::new().unwrap();
    let spec = spec(&folder, "", server(mcp_server().await, TOKEN));
    let mut agent = claude(&folder, true).open(spec).unwrap();
    let events = turn(&mut agent, "Call secret_word.").await;
    assert_eq!(events.last(), Some(&Event::Text("harbor".into())));
    assert_eq!(
        turn(&mut agent, "Thanks.").await,
        [Event::Text("Thanks.".into())]
    );
    agent.close().await;
}

#[tokio::test]
async fn ends_with_the_reason_when_the_server_refuses_its_token() {
    let folder = TempDir::new().unwrap();
    let spec = spec(&folder, "", server(mcp_server().await, "another-token"));
    let mut agent = claude(&folder, true).open(spec).unwrap();
    agent.send("Call secret_word.".into()).await.unwrap();
    let mut reason = None;
    while let Some(event) = agent.next().await {
        if let Event::Failed(Failure::Engine(text)) = event {
            reason = Some(text);
        }
    }
    let reason = reason.expect("the refused token went unreported");
    assert!(reason.contains("401"), "{reason}");
}
