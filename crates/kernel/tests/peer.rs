//! The person's conversation with an asset's peer, against an engine defined here, and the tools
//! the peer calls with the token the kernel opened it with.

mod common;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use common::{done, failed, platform};
use engine::{Agent, AgentSpec, Engine, Error, SignIn};
use kernel::{Kernel, ToolError};
use protocol::{
    AssetName, Author, Command, Conversation, Event, Failure, FailureCode, Message, Reply, TurnId,
};
use serde_json::json;
use tokio::sync::{broadcast, mpsc};

const TOOLS: &str = "http://127.0.0.1:4000/mcp";

/// An engine that records what each agent opens with. Its agents report their session first,
/// continuing the one they resume, and answer each message with two parts: the message itself and
/// a question. A message `break` makes the agent fail and end.
#[derive(Default)]
struct Scripted {
    opened: Mutex<Vec<AgentSpec>>,
}

impl Scripted {
    fn opened(&self) -> Vec<AgentSpec> {
        self.opened.lock().unwrap().clone()
    }

    /// The token the last agent opened calls the kernel's tools with.
    fn token(&self) -> String {
        self.opened().last().unwrap().mcp[0].token.clone()
    }
}

impl Engine for Scripted {
    fn open(&self, spec: AgentSpec) -> Result<Agent, Error> {
        let mut opened = self.opened.lock().unwrap();
        let session = (spec.resume.clone()).unwrap_or_else(|| format!("session-{}", opened.len()));
        opened.push(spec);
        let (input, mut messages) = mpsc::channel::<String>(1);
        let (events, receiver) = mpsc::channel(8);
        let task = tokio::spawn(async move {
            let _ = events.send(engine::Event::Started { session }).await;
            while let Some(message) = messages.recv().await {
                if message == "break" {
                    let broke = engine::Failure::Engine("the engine broke".to_owned());
                    let _ = events.send(engine::Event::Failed(broke)).await;
                    return;
                }
                for event in [
                    engine::Event::Text(message),
                    engine::Event::ToolCalled {
                        name: "mcp__federa__asset_knowledge".to_owned(),
                    },
                    engine::Event::Text("Anything else?".to_owned()),
                    engine::Event::TurnEnded,
                ] {
                    let _ = events.send(event).await;
                }
            }
        });
        Ok(Agent::new(input, receiver, task))
    }

    fn sign_in_status(&self) -> Result<SignIn, Error> {
        Ok(SignIn::SignedIn)
    }

    fn start_sign_in(&self) -> Result<(), Error> {
        Ok(())
    }

    fn stop_sign_in(&self) {}
}

fn kernel(engine: &Arc<Scripted>, conversations: &Path) -> Kernel {
    Kernel::new(
        "ana@acme.dev".to_owned(),
        Ok(engine.clone() as Arc<dyn Engine>),
        conversations.to_owned(),
        TOOLS.to_owned(),
    )
}

fn billing() -> AssetName {
    AssetName("billing".to_owned())
}

fn send(text: &str) -> Command {
    Command::SendMessage {
        asset: billing(),
        text: text.to_owned(),
    }
}

/// The organization `acme` in `dir`, with knowledge of its own and the asset `billing` registered.
async fn acme(kernel: &Kernel, dir: &Path) -> PathBuf {
    let acme = dir.join("acme");
    let create = Command::CreateOrganization {
        folder: acme.clone(),
        name: "Acme".to_owned(),
    };
    done(kernel, create).await;
    fs::write(acme.join("organization/knowledge.md"), "# Acme\n").unwrap();
    let register = Command::RegisterAsset {
        folder: platform(dir).join("services/billing"),
    };
    done(kernel, register).await;
    acme
}

/// Sends `text` to the peer of `billing`, and returns its turn.
async fn accepted(kernel: &Kernel, text: &str) -> TurnId {
    match done(kernel, send(text)).await {
        Reply::MessageAccepted { turn } => turn,
        reply => panic!("{reply:?}"),
    }
}

/// The events of `turn`, up to the one that ends it.
async fn played(events: &mut broadcast::Receiver<Event>, turn: TurnId) -> Vec<Event> {
    let mut played = Vec::new();
    loop {
        let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("the turn never ended")
            .unwrap();
        let ended = match &event {
            Event::PeerDone { turn: t, .. } | Event::PeerFailed { turn: t, .. } => *t == turn,
            _ => false,
        };
        played.push(event);
        if ended {
            return played;
        }
    }
}

fn part(turn: TurnId, text: &str) -> Event {
    Event::PeerPart {
        asset: billing(),
        turn,
        text: text.to_owned(),
    }
}

async fn conversation(kernel: &Kernel) -> Vec<Message> {
    let load = Command::LoadConversation { asset: billing() };
    match done(kernel, load).await {
        Reply::Conversation {
            conversation: Conversation { asset, messages },
        } => {
            assert_eq!(asset, billing());
            messages
        }
        reply => panic!("{reply:?}"),
    }
}

fn message(author: Author, text: &str) -> Message {
    Message {
        author,
        text: text.to_owned(),
    }
}

#[tokio::test]
async fn talks_with_the_peer_of_an_asset_and_keeps_the_conversation() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(Scripted::default());
    let conversations = dir.path().join("conversations");
    let kernel = kernel(&engine, &conversations);
    let acme = acme(&kernel, dir.path()).await;
    let mut events = kernel.subscribe();

    let first = accepted(&kernel, "What does it do?").await;
    let expected = [
        part(first, "What does it do?"),
        part(first, "Anything else?"),
        Event::PeerDone {
            asset: billing(),
            turn: first,
        },
    ];
    assert_eq!(played(&mut events, first).await, expected);

    let [spec] = &engine.opened()[..] else {
        panic!("{:?}", engine.opened());
    };
    assert!(
        spec.instructions
            .starts_with("You are the peer of the asset `billing`")
    );
    assert!(spec.instructions.contains("\n# Acme\n"));
    assert!(spec.instructions.contains("\n# billing\n"));
    let repo = fs::canonicalize(dir.path().join("platform")).unwrap();
    assert_eq!(spec.workdir, repo.join("services/billing"));
    assert_eq!(spec.mcp[0].url, TOOLS);
    assert_eq!(spec.resume, None);

    let second = accepted(&kernel, "And then?").await;
    assert_ne!(second, first);
    assert_eq!(played(&mut events, second).await.len(), 3);
    assert_eq!(engine.opened().len(), 1, "the peer kept running");
    let whole = vec![
        message(Author::Person, "What does it do?"),
        message(Author::Peer, "What does it do?\n\nAnything else?"),
        message(Author::Person, "And then?"),
        message(Author::Peer, "And then?\n\nAnything else?"),
    ];
    assert_eq!(conversation(&kernel).await, whole);

    // A runtime that starts again loads the conversation whole and continues the same session.
    let kernel = self::kernel(&engine, &conversations);
    done(&kernel, Command::OpenOrganization { folder: acme }).await;
    assert_eq!(conversation(&kernel).await, whole);
    let mut events = kernel.subscribe();
    let third = accepted(&kernel, "Still there?").await;
    played(&mut events, third).await;
    let opened = engine.opened();
    assert_eq!(opened.len(), 2);
    assert_eq!(opened[1].resume.as_deref(), Some("session-0"));
    assert_eq!(conversation(&kernel).await.len(), 6);
}

#[tokio::test]
async fn the_peer_reads_the_knowledge_of_its_own_asset() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(Scripted::default());
    let kernel = kernel(&engine, &dir.path().join("conversations"));
    acme(&kernel, dir.path()).await;
    accepted(&kernel, "Hi").await;
    let token = engine.token();

    let tools = kernel.tools(&token).unwrap();
    assert_eq!(
        tools.iter().map(|tool| tool.name).collect::<Vec<_>>(),
        ["asset_knowledge"]
    );
    let knowledge = kernel.call(&token, "asset_knowledge", json!({}));
    assert_eq!(knowledge.await, Ok("# billing\n".to_owned()));
    assert!(matches!(
        kernel.call(&token, "asset_knowledge", json!([])).await,
        Err(ToolError::Invalid(_))
    ));
    assert_eq!(
        kernel.call(&token, "deploy", json!({})).await,
        Err(ToolError::UnknownTool("deploy".to_owned()))
    );

    let knowledge = dir
        .path()
        .join("platform/services/billing/.federa/knowledge.md");
    fs::remove_file(knowledge).unwrap();
    let missing = kernel.call(&token, "asset_knowledge", json!({})).await;
    assert!(matches!(missing, Err(ToolError::Failed(message)) if message.contains("knowledge.md")));

    let guess = "0".repeat(token.len());
    assert_eq!(kernel.tools(&guess), Err(ToolError::Unauthorized));
    assert_eq!(
        kernel.call(&guess, "asset_knowledge", json!({})).await,
        Err(ToolError::Unauthorized)
    );
}

#[tokio::test]
async fn a_failed_turn_closes_the_peer_and_the_next_message_opens_it_again() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(Scripted::default());
    let kernel = kernel(&engine, &dir.path().join("conversations"));
    acme(&kernel, dir.path()).await;
    let mut events = kernel.subscribe();

    let turn = accepted(&kernel, "break").await;
    let failure = Failure {
        code: FailureCode::Internal,
        message: "the engine broke".to_owned(),
    };
    let failed = Event::PeerFailed {
        asset: billing(),
        turn,
        failure,
    };
    assert_eq!(played(&mut events, turn).await, [failed]);
    assert_eq!(kernel.tools(&engine.token()), Err(ToolError::Unauthorized));
    let asked = vec![message(Author::Person, "break")];
    assert_eq!(conversation(&kernel).await, asked);

    let turn = accepted(&kernel, "Hi").await;
    played(&mut events, turn).await;
    let opened = engine.opened();
    assert_eq!(opened.len(), 2);
    assert_eq!(opened[1].resume.as_deref(), Some("session-0"));
    assert!(kernel.tools(&engine.token()).is_ok());
}

#[tokio::test]
async fn opening_the_open_organization_keeps_its_peers_and_another_closes_them() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(Scripted::default());
    let kernel = kernel(&engine, &dir.path().join("conversations"));
    let acme = acme(&kernel, dir.path()).await;
    accepted(&kernel, "Hi").await;
    let token = engine.token();

    let spelled = acme.join("../acme");
    done(&kernel, Command::OpenOrganization { folder: spelled }).await;
    assert!(kernel.tools(&token).is_ok());
    accepted(&kernel, "Still there?").await;
    assert_eq!(engine.opened().len(), 1);

    // A failed open leaves the organization open, and its peers with it.
    let not_one = Command::OpenOrganization {
        folder: dir.path().to_owned(),
    };
    failed(&kernel, not_one).await;
    assert!(kernel.tools(&token).is_ok());

    let globex = Command::CreateOrganization {
        folder: dir.path().join("globex"),
        name: "Globex".to_owned(),
    };
    done(&kernel, globex).await;
    assert_eq!(kernel.tools(&token), Err(ToolError::Unauthorized));
    let unregistered = failed(&kernel, send("Hi")).await;
    assert_eq!(unregistered.code, FailureCode::NotFound);
    assert_eq!(engine.opened().len(), 1);
}

#[tokio::test]
async fn refuses_a_conversation_it_cannot_have() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(Scripted::default());
    let kernel = kernel(&engine, &dir.path().join("conversations"));
    let none = failed(&kernel, send("Hi")).await;
    assert_eq!(none.code, FailureCode::NoOrganization);
    let load = Command::LoadConversation { asset: billing() };
    assert_eq!(
        failed(&kernel, load.clone()).await.code,
        FailureCode::NoOrganization
    );

    let create = Command::CreateOrganization {
        folder: dir.path().join("acme"),
        name: "Acme".to_owned(),
    };
    done(&kernel, create).await;
    assert_eq!(failed(&kernel, load).await.code, FailureCode::NotFound);
    let invalid = Command::SendMessage {
        asset: AssetName("Billing".to_owned()),
        text: "Hi".to_owned(),
    };
    assert_eq!(failed(&kernel, invalid).await.code, FailureCode::Invalid);

    let without = Kernel::new(
        "ana@acme.dev".to_owned(),
        Err("claude could not be run".to_owned()),
        dir.path().join("conversations"),
        TOOLS.to_owned(),
    );
    let globex = Command::CreateOrganization {
        folder: dir.path().join("globex"),
        name: "Globex".to_owned(),
    };
    done(&without, globex).await;
    let failure = failed(&without, send("Hi")).await;
    assert_eq!(failure.code, FailureCode::Internal);
    assert_eq!(failure.message, "claude could not be run");
    assert!(engine.opened().is_empty());
}
