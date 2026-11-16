use std::{
    env, fs, io,
    net::Ipv4Addr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use serde::Deserialize;
use tokio::{
    net::TcpStream,
    time::{Instant, sleep},
};

// The runtime publishes where it listens in `runtime.toml`, in its data folder, which this side
// computes by the same rule the runtime's README gives. A runtime is running when that file exists
// and its port accepts a connection; a port that answers but refuses the token shows up when the
// relay connects. Otherwise this side starts the `runtime` binary that sits next to its own
// executable, on that same folder, and waits for it to publish.

/// How long a runtime this side started has to publish where it listens.
const STARTUP: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(50);

/// Where a running runtime listens, and the token it admits.
pub(crate) struct Endpoint {
    pub(crate) port: u16,
    pub(crate) token: String,
}

#[derive(Deserialize)]
struct Published {
    port: u16,
    token: String,
}

/// The runtime's data folder: `FEDERA_DATA_DIR`, or `federa` inside the platform's data folder.
pub(crate) fn data_folder() -> io::Result<PathBuf> {
    match env::var_os("FEDERA_DATA_DIR") {
        Some(dir) => Ok(PathBuf::from(dir)),
        None => dirs::data_dir()
            .map(|dir| dir.join("federa"))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "esta plataforma no tiene carpeta de datos; define FEDERA_DATA_DIR",
                )
            }),
    }
}

/// The person's running runtime, started first when none answers.
pub(crate) async fn reach() -> io::Result<Endpoint> {
    let binary = env::current_exe()?.with_file_name(format!("runtime{}", env::consts::EXE_SUFFIX));
    reach_in(&data_folder()?, &binary).await
}

async fn reach_in(folder: &Path, binary: &Path) -> io::Result<Endpoint> {
    if let Some(endpoint) = running(folder).await {
        return Ok(endpoint);
    }
    // Not waited on or killed: the runtime outlives the window, so agents keep working.
    let mut child = Command::new(binary)
        .env("FEDERA_DATA_DIR", folder)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("no se pudo iniciar el runtime en {}: {e}", binary.display()),
            )
        })?;
    let deadline = Instant::now() + STARTUP;
    loop {
        if let Some(endpoint) = running(folder).await {
            return Ok(endpoint);
        }
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "el runtime terminó ({status}) antes de publicar dónde escucha"
            )));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "el runtime no publicó a tiempo dónde escucha",
            ));
        }
        sleep(POLL).await;
    }
}

/// The endpoint `runtime.toml` publishes, when its port accepts a connection.
async fn running(folder: &Path) -> Option<Endpoint> {
    let text = fs::read_to_string(folder.join("runtime.toml")).ok()?;
    let published: Published = toml::from_str(&text).ok()?;
    TcpStream::connect((Ipv4Addr::LOCALHOST, published.port))
        .await
        .ok()?;
    Some(Endpoint {
        port: published.port,
        token: published.token,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    use super::*;

    async fn listener() -> (TcpListener, u16) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    fn publish(folder: &Path, port: u16, token: &str) {
        let text = format!("schema = 1\nport = {port}\ntoken = \"{token}\"\n");
        fs::write(folder.join("runtime.toml"), text).unwrap();
    }

    #[tokio::test]
    async fn reaches_the_runtime_that_answers_where_it_published() {
        let folder = TempDir::new().unwrap();
        let (_listener, port) = listener().await;
        publish(folder.path(), port, "secret");

        let missing = folder.path().join("no-runtime");
        let endpoint = reach_in(folder.path(), &missing).await.unwrap();

        assert_eq!(endpoint.port, port);
        assert_eq!(endpoint.token, "secret");
    }

    #[tokio::test]
    async fn fails_to_start_a_runtime_that_is_not_there() {
        let folder = TempDir::new().unwrap();
        let missing = folder.path().join("no-runtime");

        let error = reach_in(folder.path(), &missing).await.err().unwrap();

        assert!(error.to_string().contains("no se pudo iniciar el runtime"));
    }

    #[cfg(unix)]
    mod started {
        use std::os::unix::fs::PermissionsExt;

        use tokio::sync::Mutex;

        use super::*;

        // Linux refuses to run a script while another thread's fork still holds it open for
        // writing, so the tests that write a fake runtime and start it take turns.
        static STARTING: Mutex<()> = Mutex::const_new(());

        /// A fake `runtime` that runs `script` with the data folder it was handed.
        fn fake_runtime(dir: &Path, script: &str) -> PathBuf {
            let binary = dir.join("runtime");
            fs::write(&binary, format!("#!/bin/sh\n{script}\n")).unwrap();
            fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
            binary
        }

        #[tokio::test]
        async fn starts_the_runtime_when_the_published_one_does_not_answer() {
            let folder = TempDir::new().unwrap();
            // Nothing ever accepts a connection on port 0, as on the port of a runtime that ended.
            publish(folder.path(), 0, "old");
            let (_listener, port) = listener().await;
            let binary = fake_runtime(
                folder.path(),
                &format!(
                    "printf 'schema = 1\\nport = {port}\\ntoken = \"new\"\\n' \
                     > \"$FEDERA_DATA_DIR/runtime.toml\""
                ),
            );

            let endpoint = {
                let _turn = STARTING.lock().await;
                reach_in(folder.path(), &binary).await.unwrap()
            };

            assert_eq!(endpoint.port, port);
            assert_eq!(endpoint.token, "new");
        }

        #[tokio::test]
        async fn reports_a_runtime_that_exits_before_it_publishes() {
            let folder = TempDir::new().unwrap();
            let binary = fake_runtime(folder.path(), "exit 3");

            let error = {
                let _turn = STARTING.lock().await;
                reach_in(folder.path(), &binary).await.err().unwrap()
            };

            assert!(error.to_string().contains("terminó"), "{error}");
        }
    }
}
