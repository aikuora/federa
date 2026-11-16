use std::{
    fs,
    path::Path,
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use protocol::{
    ClientFrame, Command as ProtocolCommand, Hello, Organization, Outcome, Reply, RequestId,
    RuntimeFrame, VERSION,
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

/// The runtime on `dir`, with `gitconfig` as git's global configuration.
fn command(dir: &Path, gitconfig: &str) -> Command {
    let file = dir.join("gitconfig");
    fs::write(&file, gitconfig).unwrap();
    let mut runtime = Command::new(env!("CARGO_BIN_EXE_runtime"));
    runtime
        .env("FEDERA_DATA_DIR", dir.join("data"))
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
    let mut child = command(data.path(), gitconfig)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let file = data.path().join("data/runtime.toml");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !file.exists() {
        assert!(child.try_wait().unwrap().is_none(), "the runtime exited");
        assert!(Instant::now() < deadline, "no runtime.toml");
        thread::sleep(Duration::from_millis(20));
    }
    let published = toml::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    Runtime {
        child,
        published,
        data,
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

fn refused_status(result: Result<Socket, tungstenite::Error>) -> StatusCode {
    match result {
        Err(tungstenite::Error::Http(response)) => response.status(),
        Err(error) => panic!("{error}"),
        Ok(_) => panic!("connected"),
    }
}

#[cfg(unix)]
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
