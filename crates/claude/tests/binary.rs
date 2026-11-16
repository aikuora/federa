//! The engine against stand-ins for the binary, for the cases the fake `claude` does not play:
//! shell scripts that print a version and answer each stdin line with one turn of stream-json, or
//! whose sign-in ends late or never.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use claude::Claude;
use engine::{AgentSpec, Engine, Error, Event, Failure, SignIn};

const TURNS: &str = r#"while read -r line; do
  echo '{"type":"system","subtype":"init","session_id":"s1"}'
  echo '{"type":"assistant","message":{"content":[{"type":"text","text":"ok"}]}}'
  echo '{"type":"result","subtype":"success","is_error":false,"result":"ok"}'
done"#;

// Answers its one turn with the configuration folder it was given.
const CONFIG_ECHO: &str = r#"read -r line
echo "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"$CLAUDE_CONFIG_DIR\"}]}}"
echo '{"type":"result","subtype":"success","is_error":false}'"#;

// Its sign-in signs the folder in, and only a second later writes that it ended.
const LINGERING: &str = r#"if [ "$1 $2" = "auth status" ]; then test -e "$CLAUDE_CONFIG_DIR/signed-in"; exit; fi
touch "$CLAUDE_CONFIG_DIR/signed-in"; sleep 1; touch "$CLAUDE_CONFIG_DIR/ended""#;

// Its sign-in writes its process id and never ends.
const HANGING: &str = r#"echo $$ > "$CLAUDE_CONFIG_DIR/login.pid"; exec sleep 60"#;

const CURRENT: &str = "2.1.311 (Claude Code)";

// Every stand-in, as its name, the version it prints and what it does with its arguments and stdin.
const STAND_INS: [(&str, &str, &str); 7] = [
    ("old", "2.1.310 (Claude Code)", TURNS),
    ("newer", "2.2.0 (Claude Code)", TURNS),
    ("odd", "usage: claude", TURNS),
    (
        "dying",
        CURRENT,
        "read -r line; echo 'session expired' >&2; exit 3",
    ),
    ("config-echo", CURRENT, CONFIG_ECHO),
    ("lingering", CURRENT, LINGERING),
    ("hanging", CURRENT, HANGING),
];

// All of them are written before any test runs one: on Linux, running a file while another thread
// still holds it open for writing, even through a fork, fails with ETXTBSY.
fn stand_in(name: &str) -> PathBuf {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let folder = FOLDER.get_or_init(|| {
        let folder = std::env::temp_dir().join(format!("claude-stand-in-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        for (name, version, body) in STAND_INS {
            let path = folder.join(name);
            let script = format!(
                "#!/bin/sh\nif [ \"$1\" = --version ]; then echo \"{version}\"; exit 0; fi\n{body}\n"
            );
            std::fs::write(&path, script).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        folder
    });
    folder.join(name)
}

#[test]
fn refuses_a_binary_older_than_the_minimum() {
    let error = Claude::new(stand_in("old"), config()).err();
    let Some(Error::TooOld { found, minimum }) = error else {
        panic!("an old binary was accepted: {error:?}");
    };
    assert_eq!((found.as_str(), minimum.as_str()), ("2.1.310", "2.1.311"));
}

#[test]
fn accepts_a_newer_binary_without_notice() {
    assert!(Claude::new(stand_in("newer"), config()).is_ok());
}

#[test]
fn says_a_missing_binary_is_not_installed() {
    let missing = PathBuf::from("/nonexistent/claude");
    let error = Claude::new(missing.clone(), config()).err();
    assert!(matches!(error, Some(Error::NotInstalled(path)) if path == missing));
}

#[test]
fn refuses_a_binary_that_prints_no_version() {
    let error = Claude::new(stand_in("odd"), config()).err();
    assert!(matches!(error, Some(Error::Io(_))), "{error:?}");
}

fn config() -> PathBuf {
    std::env::temp_dir().join("claude-stand-in-config")
}

fn spec() -> AgentSpec {
    AgentSpec {
        instructions: "You are a test agent.".into(),
        workdir: std::env::temp_dir(),
        mcp: vec![],
    }
}

#[tokio::test]
async fn reports_a_binary_that_ends_on_its_own_with_what_it_said() {
    let claude = Claude::new(stand_in("dying"), config()).unwrap();
    let mut agent = claude.open(spec()).unwrap();
    agent.send("one".into()).await.unwrap();
    let Some(Event::Failed(Failure::Engine(reason))) = agent.next().await else {
        panic!("the end of the binary went unreported");
    };
    assert!(reason.contains("session expired"), "{reason}");
    assert_eq!(agent.next().await, None);
    assert!(matches!(agent.send("two".into()).await, Err(Error::Closed)));
}

#[tokio::test]
async fn runs_the_binary_on_an_absolute_configuration_folder() {
    // Relative to the crate, where tests run, and inside the ignored `target/`.
    let relative = PathBuf::from("../../target/claude-relative-config");
    let claude = Claude::new(stand_in("config-echo"), relative.clone()).unwrap();
    let mut agent = claude.open(spec()).unwrap();
    agent.send("which folder?".into()).await.unwrap();
    let seen = agent.next().await;
    agent.close().await;
    let absolute = std::path::absolute(&relative).unwrap();
    std::fs::remove_dir_all(&absolute).unwrap();
    assert_eq!(seen, Some(Event::Text(absolute.display().to_string())));
}

/// A configuration folder no other test uses, with nothing in it.
fn fresh_config(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("claude-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

fn wait_for(what: &str, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn running(pid: &str) -> bool {
    let probe = Command::new("kill").args(["-0", pid]).output().unwrap();
    probe.status.success()
}

fn login_pid(folder: &Path) -> String {
    let file = folder.join("login.pid");
    wait_for("the sign-in's start", || file.exists());
    std::fs::read_to_string(&file).unwrap().trim().to_owned()
}

#[test]
fn lets_a_sign_in_end_on_its_own_once_signed_in() {
    let folder = fresh_config("lingering");
    let claude = Claude::new(stand_in("lingering"), folder.clone()).unwrap();
    claude.start_sign_in().unwrap();
    wait_for("the sign-in", || {
        claude.sign_in_status().unwrap() == SignIn::SignedIn
    });
    claude.stop_sign_in();
    wait_for("the sign-in's own end", || folder.join("ended").exists());
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn ends_a_sign_in_still_running_when_stopped_or_started_again() {
    let folder = fresh_config("hanging");
    let claude = Claude::new(stand_in("hanging"), folder.clone()).unwrap();
    claude.start_sign_in().unwrap();
    let first = login_pid(&folder);
    std::fs::remove_file(folder.join("login.pid")).unwrap();
    claude.start_sign_in().unwrap();
    let second = login_pid(&folder);
    assert!(!running(&first), "the first sign-in still runs");
    assert!(running(&second));
    claude.stop_sign_in();
    wait_for("the end of the stopped sign-in", || !running(&second));
    claude.stop_sign_in();
    std::fs::remove_dir_all(folder).unwrap();
}
