//! The runtime binary as a client sees it. It runs the fake `claude`, a shell launcher, so these
//! run where a shell does.
#![cfg(unix)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use protocol::{
    AssetName, Author, ClientFrame, Command as ProtocolCommand, Event, Hello, Organization,
    Outcome, Reply, RequestId, RuntimeFrame, SignInStatus, VERSION,
};
use serde::Deserialize;
use tempfile::TempDir;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{self, Message, client::IntoClientRequest, http::StatusCode},
};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Deserialize)]
struct Published {
    schema: u32,
    port: u16,
    token: String,
}

/// The runtime binary, run on a data folder and a git configuration of its own, so nothing of the
/// person's setup reaches it. Killed when dropped.
struct Runtime {
    child: Child,
    published: Published,
    data: TempDir,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

const IDENTITY: &str = "[user]\n\temail = ada@example.com\n";

/// The folder of the fake `claude`, which every runtime here finds first on `PATH`, so the real one
/// never runs.
fn fake_claude() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/claude/fake")
}

/// The runtime on `dir`, with `gitconfig` as git's global configuration.
fn command(dir: &Path, gitconfig: &str) -> Command {
    let file = dir.join("gitconfig");
    fs::write(&file, gitconfig).unwrap();
    let path = env::var_os("PATH").unwrap_or_default();
    let folders = [fake_claude()].into_iter().chain(env::split_paths(&path));
    let mut runtime = Command::new(env!("CARGO_BIN_EXE_runtime"));
    runtime
        .env("FEDERA_DATA_DIR", dir.join("data"))
        .env("PATH", env::join_paths(folders).unwrap())
        .env("GIT_CONFIG_GLOBAL", &file)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    runtime
}

fn start() -> Runtime {
    start_with(IDENTITY)
}

fn start_with(gitconfig: &str) -> Runtime {
    let data = tempfile::tempdir().unwrap();
    let command = command(data.path(), gitconfig);
    launch(data, command)
}

/// A runtime that finds git on its `PATH` and no `claude`.
fn start_without_claude() -> Runtime {
    let data = tempfile::tempdir().unwrap();
    let path = env::var_os("PATH").unwrap_or_default();
    let git = env::split_paths(&path)
        .map(|folder| folder.join("git"))
        .find(|git| git.exists())
        .unwrap();
    let only_git = data.path().join("bin");
    fs::create_dir(&only_git).unwrap();
    std::os::unix::fs::symlink(git, only_git.join("git")).unwrap();
    let mut command = command(data.path(), IDENTITY);
    command.env("PATH", &only_git);
    launch(data, command)
}

fn launch(data: TempDir, command: Command) -> Runtime {
    let (child, published) = run(data.path(), command);
    Runtime {
        child,
        published,
        data,
    }
}

/// Runs `command`, the runtime on `dir`, until it publishes where to connect.
fn run(dir: &Path, mut command: Command) -> (Child, Published) {
    let mut child = command.stderr(Stdio::null()).spawn().unwrap();
    let file = dir.join("data/runtime.toml");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !file.exists() {
        assert!(child.try_wait().unwrap().is_none(), "the runtime exited");
        assert!(Instant::now() < deadline, "no runtime.toml");
        thread::sleep(Duration::from_millis(20));
    }
    let published = toml::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    (child, published)
}

impl Runtime {
    /// Stops the runtime and starts another on the same data folder, as when the person closes
    /// the app and opens it again.
    fn restart(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        fs::remove_file(self.data.path().join("data/runtime.toml")).unwrap();
        (self.child, self.published) = run(self.data.path(), command(self.data.path(), IDENTITY));
    }
}

async fn connect(runtime: &Runtime, token: Option<&str>) -> Result<Socket, tungstenite::Error> {
    let url = format!("ws://127.0.0.1:{}/", runtime.published.port);
    let mut request = url.into_client_request().unwrap();
    if let Some(token) = token {
        let bearer = format!("Bearer {token}").parse().unwrap();
        request.headers_mut().insert("authorization", bearer);
    }
    connect_async(request).await.map(|(socket, _)| socket)
}

async fn send(socket: &mut Socket, frame: ClientFrame) {
    let text = serde_json::to_string(&frame).unwrap();
    socket.send(Message::text(text)).await.unwrap();
}

/// The next frame, or `None` once the runtime closed the connection.
async fn receive(socket: &mut Socket) -> Option<RuntimeFrame> {
    match socket.next().await? {
        Ok(Message::Text(text)) => Some(serde_json::from_str(&text).unwrap()),
        Ok(Message::Close(_)) | Err(_) => None,
        Ok(other) => panic!("{other:?}"),
    }
}

/// A connection whose `Hello` the runtime already answered.
async fn greeted(runtime: &Runtime) -> Socket {
    let mut socket = connect(runtime, Some(&runtime.published.token))
        .await
        .unwrap();
    send(&mut socket, ClientFrame::Hello(Hello { protocol: VERSION })).await;
    let hello = receive(&mut socket).await;
    assert_eq!(
        hello,
        Some(RuntimeFrame::Hello(Hello { protocol: VERSION }))
    );
    socket
}

/// Sends `command` and returns its reply, which must be the next frame.
async fn ask(socket: &mut Socket, id: u32, command: ProtocolCommand) -> Outcome {
    send(
        socket,
        ClientFrame::Command {
            id: RequestId(id),
            command,
        },
    )
    .await;
    match receive(socket).await {
        Some(RuntimeFrame::Reply {
            id: replied,
            outcome,
        }) if replied == RequestId(id) => outcome,
        frame => panic!("{frame:?}"),
    }
}

async fn sign_in_status(socket: &mut Socket, id: u32, command: ProtocolCommand) -> SignInStatus {
    match ask(socket, id, command).await {
        Outcome::Done {
            reply: Reply::SignIn { status },
        } => status,
        outcome => panic!("{outcome:?}"),
    }
}

fn refused_status(result: Result<Socket, tungstenite::Error>) -> StatusCode {
    match result {
        Err(tungstenite::Error::Http(response)) => response.status(),
        Err(error) => panic!("{error}"),
        Ok(_) => panic!("connected"),
    }
}

#[test]
fn publishes_where_to_connect_readable_only_by_its_owner() {
    use std::os::unix::fs::PermissionsExt;

    let runtime = start();
    assert_eq!(runtime.published.schema, 1);
    let file = runtime.data.path().join("data/runtime.toml");
    let mode = fs::metadata(file).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[tokio::test]
async fn a_client_with_the_token_creates_an_organization() {
    let runtime = start();
    let mut socket = connect(&runtime, Some(&runtime.published.token))
        .await
        .unwrap();
    send(&mut socket, ClientFrame::Hello(Hello { protocol: VERSION })).await;
    assert_eq!(
        receive(&mut socket).await,
        Some(RuntimeFrame::Hello(Hello { protocol: VERSION })),
    );

    let folder = runtime.data.path().join("acme");
    send(
        &mut socket,
        ClientFrame::Command {
            id: RequestId(1),
            command: ProtocolCommand::CreateOrganization {
                folder: folder.clone(),
                name: "Acme".into(),
            },
        },
    )
    .await;
    let Some(RuntimeFrame::Reply { id, outcome }) = receive(&mut socket).await else {
        panic!("no reply");
    };
    assert_eq!(id, RequestId(1));
    let Outcome::Done {
        reply: Reply::Organization { organization },
    } = outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!(
        organization,
        Organization {
            name: "Acme".into(),
            folder,
        },
    );
}

#[tokio::test]
async fn a_client_without_the_token_is_refused() {
    let runtime = start();
    assert_eq!(
        refused_status(connect(&runtime, None).await),
        StatusCode::UNAUTHORIZED
    );
    let guess = "0".repeat(runtime.published.token.len());
    assert_eq!(
        refused_status(connect(&runtime, Some(&guess)).await),
        StatusCode::UNAUTHORIZED,
    );
}

#[tokio::test]
async fn a_client_of_another_version_gets_the_runtimes_and_is_closed() {
    let runtime = start();
    let mut socket = connect(&runtime, Some(&runtime.published.token))
        .await
        .unwrap();
    send(
        &mut socket,
        ClientFrame::Hello(Hello {
            protocol: VERSION + 1,
        }),
    )
    .await;
    assert_eq!(
        receive(&mut socket).await,
        Some(RuntimeFrame::Hello(Hello { protocol: VERSION })),
    );
    assert_eq!(receive(&mut socket).await, None);
}

#[tokio::test]
async fn a_command_before_the_hello_closes_the_connection() {
    let runtime = start();
    let mut socket = connect(&runtime, Some(&runtime.published.token))
        .await
        .unwrap();
    send(
        &mut socket,
        ClientFrame::Command {
            id: RequestId(1),
            command: ProtocolCommand::ListAssets,
        },
    )
    .await;
    assert_eq!(receive(&mut socket).await, None);
}

#[test]
fn refuses_to_start_without_a_git_email() {
    let dir = tempfile::tempdir().unwrap();
    let Output { status, stderr, .. } = command(dir.path(), "").output().unwrap();
    assert!(!status.success());
    assert!(String::from_utf8_lossy(&stderr).contains("git config --global user.email"));
    assert!(!dir.path().join("data/runtime.toml").exists());
}

#[test]
fn takes_the_email_from_a_file_the_global_configuration_includes() {
    let included = tempfile::tempdir().unwrap();
    let identity = included.path().join("identity");
    fs::write(&identity, IDENTITY).unwrap();
    let runtime = start_with(&format!("[include]\n\tpath = {}\n", identity.display()));
    assert_eq!(runtime.published.schema, 1);
}

#[tokio::test]
async fn a_client_signs_claude_in_and_hears_when_it_is() {
    let runtime = start();
    let mut socket = greeted(&runtime).await;
    let status = sign_in_status(&mut socket, 1, ProtocolCommand::SignInStatus).await;
    assert_eq!(status, SignInStatus::SignedOut);
    // The reply comes before the event the command caused.
    let status = sign_in_status(&mut socket, 2, ProtocolCommand::StartSignIn).await;
    assert_eq!(status, SignInStatus::SigningIn);
    for status in [SignInStatus::SigningIn, SignInStatus::SignedIn] {
        let event = Event::SignInChanged { status };
        assert_eq!(
            receive(&mut socket).await,
            Some(RuntimeFrame::Event { event })
        );
    }
    let status = sign_in_status(&mut socket, 3, ProtocolCommand::SignInStatus).await;
    assert_eq!(status, SignInStatus::SignedIn);
}

#[tokio::test]
async fn without_claude_serves_organizations_and_says_why_agents_cannot_run() {
    let runtime = start_without_claude();
    let mut socket = greeted(&runtime).await;
    let status = sign_in_status(&mut socket, 1, ProtocolCommand::SignInStatus).await;
    let SignInStatus::Unavailable { reason } = status else {
        panic!("{status:?}");
    };
    assert!(reason.contains("claude could not be run"), "{reason}");
    let create = ProtocolCommand::CreateOrganization {
        folder: runtime.data.path().join("acme"),
        name: "Acme".into(),
    };
    let outcome = ask(&mut socket, 2, create).await;
    assert!(matches!(outcome, Outcome::Done { .. }), "{outcome:?}");
}

const KNOWLEDGE: &str = "# billing\n\nCharges each customer once a month.\n";

/// A git repository in `dir` holding the asset `billing`, with `KNOWLEDGE` in its `.federa/`.
fn billing_repository(dir: &Path) -> PathBuf {
    let billing = dir.join("billing");
    let federa = billing.join(".federa");
    fs::create_dir_all(&federa).unwrap();
    fs::write(
        federa.join("asset.toml"),
        "schema = 1\nname = \"billing\"\n",
    )
    .unwrap();
    fs::write(federa.join("knowledge.md"), KNOWLEDGE).unwrap();
    // A git hook sets these for the repository it runs in, and git would init that one instead.
    let init = Command::new("git")
        .arg("init")
        .arg(&billing)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .unwrap();
    assert!(init.status.success());
    billing
}

/// Signs the configuration folder of the runtime's agents in, through the fake's `auth login`.
fn sign_in(runtime: &Runtime) {
    let status = Command::new(fake_claude().join("claude"))
        .args(["auth", "login"])
        .env("CLAUDE_CONFIG_DIR", runtime.data.path().join("data/claude"))
        .status()
        .unwrap();
    assert!(status.success());
}

fn billing() -> AssetName {
    AssetName("billing".into())
}

/// Asks the peer of `billing` and returns its answer: the parts of its turn, joined.
async fn answer(socket: &mut Socket, id: u32, text: &str) -> String {
    let send = ProtocolCommand::SendMessage {
        asset: billing(),
        text: text.into(),
    };
    let Outcome::Done {
        reply: Reply::MessageAccepted { turn },
    } = ask(socket, id, send).await
    else {
        panic!("the message was not accepted");
    };
    let mut parts = Vec::new();
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(20), receive(socket))
            .await
            .expect("the turn never ended");
        match frame {
            Some(RuntimeFrame::Event {
                event: Event::PeerPart { turn: t, text, .. },
            }) if t == turn => parts.push(text),
            Some(RuntimeFrame::Event {
                event: Event::PeerDone { turn: t, .. },
            }) if t == turn => return parts.join("\n\n"),
            frame => panic!("{frame:?}"),
        }
    }
}

async fn conversation(socket: &mut Socket, id: u32) -> Vec<protocol::Message> {
    let load = ProtocolCommand::LoadConversation { asset: billing() };
    match ask(socket, id, load).await {
        Outcome::Done {
            reply: Reply::Conversation { conversation },
        } => conversation.messages,
        outcome => panic!("{outcome:?}"),
    }
}

/// The engine's session stored with the conversation about `billing`, the only one there is.
fn stored_session(runtime: &Runtime) -> String {
    let conversations = runtime.data.path().join("data/conversations");
    let organizations: Vec<_> = fs::read_dir(conversations).unwrap().collect();
    let [organization] = &organizations[..] else {
        panic!("{organizations:?}");
    };
    let file = organization.as_ref().unwrap().path().join("billing.toml");
    let stored: toml::Table = toml::from_str(&fs::read_to_string(file).unwrap()).unwrap();
    stored["session"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn the_peer_answers_with_its_assets_knowledge_and_keeps_the_conversation_across_restarts() {
    let mut runtime = start();
    sign_in(&runtime);
    let acme = runtime.data.path().join("acme");
    let billing = billing_repository(runtime.data.path());
    let mut socket = greeted(&runtime).await;
    let create = ProtocolCommand::CreateOrganization {
        folder: acme.clone(),
        name: "Acme".into(),
    };
    let register = ProtocolCommand::RegisterAsset { folder: billing };
    for (id, command) in [(1, create), (2, register)] {
        let outcome = ask(&mut socket, id, command).await;
        assert!(matches!(outcome, Outcome::Done { .. }), "{outcome:?}");
    }

    // The fake calls the tool its instructions name, and answers with what the tool returned.
    let question = "What does billing do?";
    assert_eq!(answer(&mut socket, 3, question).await, KNOWLEDGE);
    let session = stored_session(&runtime);

    runtime.restart();
    let mut socket = greeted(&runtime).await;
    let open = ProtocolCommand::OpenOrganization { folder: acme };
    let outcome = ask(&mut socket, 1, open).await;
    assert!(matches!(outcome, Outcome::Done { .. }), "{outcome:?}");
    let whole = vec![
        protocol::Message {
            author: Author::Person,
            text: question.into(),
        },
        protocol::Message {
            author: Author::Peer,
            text: KNOWLEDGE.into(),
        },
    ];
    assert_eq!(conversation(&mut socket, 2).await, whole);
    assert_eq!(answer(&mut socket, 3, "And what else?").await, KNOWLEDGE);
    assert_eq!(stored_session(&runtime), session);
    assert_eq!(conversation(&mut socket, 4).await.len(), 4);
}
