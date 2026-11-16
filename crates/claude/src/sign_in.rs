use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use engine::SignIn;

use crate::launch;

// Agents run on a configuration folder of their own, so signing in from the person's terminal
// never reaches them; only the app starts the official sign-in on that folder.

/// What the person does when an agent finds its configuration folder not signed in.
pub(crate) const HINT: &str =
    "connect Claude from the app, which opens Claude's official sign-in for its own configuration";

/// Whether `config` is signed in, by `claude auth status`, whose output is never read beyond its
/// exit code.
pub(crate) fn status(binary: &Path, config: &Path) -> io::Result<SignIn> {
    // With the agents' environment, not the runtime's: a credential the runtime has, such as
    // `ANTHROPIC_API_KEY`, never reaches an agent, so it must not make the folder look signed in.
    let status = Command::new(binary)
        .args(["auth", "status"])
        .env_clear()
        .envs(launch::environment(config))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(if status.success() {
        SignIn::SignedIn
    } else {
        SignIn::SignedOut
    })
}

/// Starts `claude auth login` on `config`, with no terminal: the binary opens the provider's page in
/// the browser and ends once the person has signed in.
pub(crate) fn login(binary: &Path, config: &Path) -> io::Result<Child> {
    // The person's own sign-in, not an agent, so it keeps the runtime's environment, which the
    // browser needs to open.
    Command::new(binary)
        .args(["auth", "login"])
        .env("CLAUDE_CONFIG_DIR", config)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// How long a sign-in that signed the folder in has to end on its own.
const GRACE: Duration = Duration::from_secs(3);

/// Lets `login` end on its own within `GRACE`, and ends it after that. The folder may read signed in
/// before the binary has written all it writes, which killing it at once would cut off.
pub(crate) fn finish(mut login: Child) {
    let deadline = Instant::now() + GRACE;
    while matches!(login.try_wait(), Ok(None)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    end(login);
}

/// Ends `login` at once and reaps it. One that already ended is only reaped.
pub(crate) fn end(mut login: Child) {
    let _ = login.kill();
    let _ = login.wait();
}
