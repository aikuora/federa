use engine::{Event, Failure};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
};
use tokio::process::Child;
use tokio::sync::mpsc;

use crate::{launch, stream};

/// Runs a started binary until it exits: writes each message to its stdin, reports what its
/// stdout says, and closes its stdin once `messages` closes so it finishes and exits.
pub(crate) async fn run(
    mut child: Child,
    messages: mpsc::Receiver<String>,
    events: mpsc::Sender<Event>,
) {
    let (Some(stdin), Some(stdout), Some(mut stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        unreachable!("the launch command pipes stdin, stdout and stderr");
    };
    // Read alongside stdout, so a binary that writes a lot to stderr never blocks on a full pipe.
    let errors = tokio::spawn(async move { tail(&mut stderr).await });
    let mut reporter = Reporter::new(events);
    let stdout = tokio::io::BufReader::new(stdout);
    let closed = converse(stdin, stdout, messages, &mut reporter).await;
    let status = child.wait().await;
    let errors = errors.await.unwrap_or_default();
    let exited_well = matches!(&status, Ok(status) if status.success());
    if !closed || !exited_well {
        let status = status.map_or_else(|error| error.to_string(), |status| status.to_string());
        let reason = format!("claude ended ({status}): {}", errors.trim());
        reporter
            .report(Event::Failed(Failure::Engine(reason)))
            .await;
    }
}

// How much of the end of stderr a failure keeps; the binary may write to it all its life.
const STDERR_TAIL: usize = 64 * 1024;

/// The last `STDERR_TAIL` bytes `reader` gives until it ends.
async fn tail(reader: &mut (impl AsyncRead + Unpin)) -> String {
    let mut kept = Vec::new();
    let mut chunk = [0; 8192];
    while let Ok(read @ 1..) = reader.read(&mut chunk).await {
        kept.extend_from_slice(&chunk[..read]);
        let excess = kept.len().saturating_sub(STDERR_TAIL);
        kept.drain(..excess);
    }
    String::from_utf8_lossy(&kept).into_owned()
}

/// Reports every event on `stdout` until it ends, and says whether `messages` had closed by then;
/// if not, the binary ended on its own.
pub(crate) async fn converse(
    stdin: impl AsyncWrite + Unpin,
    stdout: impl AsyncBufRead + Unpin,
    mut messages: mpsc::Receiver<String>,
    reporter: &mut Reporter,
) -> bool {
    let mut stdin = Some(stdin);
    let mut lines = stdout.lines();
    loop {
        tokio::select! {
            message = messages.recv(), if stdin.is_some() => match (message, stdin.as_mut()) {
                (Some(text), Some(pipe)) => {
                    let line = launch::user_message(&text);
                    // A pipe that fails means the binary is gone, and its stdout ends next.
                    if pipe.write_all(line.as_bytes()).await.is_err() || pipe.flush().await.is_err() {
                        stdin = None;
                    }
                }
                // Dropping stdin closes it, which tells the binary to finish.
                _ => stdin = None,
            },
            line = lines.next_line() => match line {
                Ok(Some(line)) => match stream::read(&line) {
                    Ok(events) => {
                        for event in events {
                            reporter.report(event).await;
                        }
                    }
                    Err(failure) => reporter.report(Event::Failed(failure)).await,
                },
                Ok(None) => return stdin.is_none(),
                Err(error) => {
                    let failure = Failure::Unreadable(error.to_string());
                    reporter.report(Event::Failed(failure)).await;
                    return stdin.is_none();
                }
            },
        }
    }
}

/// Sends events to the agent's owner, once each session id.
pub(crate) struct Reporter {
    events: mpsc::Sender<Event>,
    session: Option<String>,
}

impl Reporter {
    pub(crate) fn new(events: mpsc::Sender<Event>) -> Self {
        Self {
            events,
            session: None,
        }
    }

    async fn report(&mut self, event: Event) {
        // The binary repeats its init line on every turn; the owner hears the session once.
        if let Event::Started { session } = &event {
            if self.session.as_ref() == Some(session) {
                return;
            }
            self.session = Some(session.clone());
        }
        // An owner that stopped listening has closed the agent, which still finishes on its own.
        let _ = self.events.send(event).await;
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncWriteExt, BufReader};

    use super::*;

    struct Replay {
        events: Vec<Event>,
        closed: bool,
        stdin: String,
    }

    // Plays the binary: takes every message, and only then prints `stdout`, as the real one does
    // once its stdin closes. With `keep_open`, the binary ends while its owner still talks to it.
    async fn replay(stdout: &str, messages: &[&str], keep_open: bool) -> Replay {
        let (stdin_end, mut stdin) = tokio::io::duplex(1 << 20);
        let (mut stdout_end, stdout_pipe) = tokio::io::duplex(1 << 20);
        let (input, received) = mpsc::channel(8);
        let (events, mut reported) = mpsc::channel(1024);
        let session = tokio::spawn(async move {
            let mut reporter = Reporter::new(events);
            converse(
                stdin_end,
                BufReader::new(stdout_pipe),
                received,
                &mut reporter,
            )
            .await
        });
        for message in messages {
            input.send(message.to_string()).await.unwrap();
        }
        let mut written = String::new();
        if keep_open {
            let expected = messages.iter().map(|m| launch::user_message(m).len()).sum();
            let mut buffer = vec![0; expected];
            stdin.read_exact(&mut buffer).await.unwrap();
            written = String::from_utf8(buffer).unwrap();
        } else {
            drop(input);
            stdin.read_to_string(&mut written).await.unwrap();
        }
        stdout_end.write_all(stdout.as_bytes()).await.unwrap();
        drop(stdout_end);
        let closed = session.await.unwrap();
        let mut events = Vec::new();
        while let Some(event) = reported.recv().await {
            events.push(event);
        }
        Replay {
            events,
            closed,
            stdin: written,
        }
    }

    const TURN: &str = concat!(
        r#"{"type":"system","subtype":"init","session_id":"s1"}"#,
        "\n",
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"ok"}]}}"#,
        "\n",
        r#"{"type":"result","subtype":"success","is_error":false,"result":"ok"}"#,
        "\n",
    );

    #[tokio::test]
    async fn writes_each_message_as_a_line_on_stdin() {
        let replay = replay(TURN, &["one", "two"], false).await;
        let expected = launch::user_message("one") + &launch::user_message("two");
        assert_eq!(replay.stdin, expected);
        assert!(replay.closed);
    }

    #[tokio::test]
    async fn reports_the_session_once_though_every_turn_repeats_it() {
        let replay = replay(&TURN.repeat(2), &["one", "two"], false).await;
        let turn = [Event::Text("ok".into()), Event::TurnEnded];
        let started = Event::Started {
            session: "s1".into(),
        };
        let expected: Vec<_> = [started]
            .into_iter()
            .chain(turn.clone())
            .chain(turn)
            .collect();
        assert_eq!(replay.events, expected);
    }

    #[tokio::test]
    async fn reports_an_unreadable_line_and_reads_on() {
        let stdout = format!("garbage\n{TURN}");
        let replay = replay(&stdout, &["one"], false).await;
        assert!(matches!(
            replay.events[0],
            Event::Failed(Failure::Unreadable(_))
        ));
        assert_eq!(replay.events.last(), Some(&Event::TurnEnded));
    }

    // The recordings of the minimum version, made by `recordings/record.ts`.
    async fn replay_recording(recording: &str, messages: &[&str]) -> Vec<Event> {
        let replay = replay(recording, messages, false).await;
        assert!(replay.closed);
        replay.events
    }

    fn started(events: &[Event]) -> &[Event] {
        let Some((Event::Started { session }, rest)) = events.split_first() else {
            panic!("no session at the start: {events:?}");
        };
        assert!(!session.is_empty());
        rest
    }

    #[tokio::test]
    async fn opens_a_session_on_the_recording() {
        let recording = include_str!("../recordings/open.jsonl");
        let events = replay_recording(recording, &["ready?"]).await;
        let turn = [Event::Text("ready".into()), Event::TurnEnded];
        assert_eq!(started(&events), turn);
    }

    #[tokio::test]
    async fn converses_two_turns_on_the_recording() {
        let recording = include_str!("../recordings/two-turns.jsonl");
        let events = replay_recording(recording, &["remember", "recall"]).await;
        let turns = [
            Event::Text("ok".into()),
            Event::TurnEnded,
            Event::Text("orange".into()),
            Event::TurnEnded,
        ];
        assert_eq!(started(&events), turns);
    }

    #[tokio::test]
    async fn calls_an_mcp_tool_on_the_recording() {
        let recording = include_str!("../recordings/mcp-tool.jsonl");
        let events = replay_recording(recording, &["call it"]).await;
        // The model may look the tool up first, or call it straight away.
        let searched = Event::ToolCalled {
            name: "ToolSearch".into(),
        };
        let turn: Vec<_> = started(&events)
            .iter()
            .filter(|event| **event != searched)
            .cloned()
            .collect();
        let called = Event::ToolCalled {
            name: "mcp__recorder__secret_word".into(),
        };
        let expected = [called, Event::Text("lighthouse".into()), Event::TurnEnded];
        assert_eq!(turn, expected);
    }

    #[tokio::test]
    async fn tells_to_sign_in_on_the_recording_not_signed_in() {
        let recording = include_str!("../recordings/not-signed-in.jsonl");
        let events = replay_recording(recording, &["ready?"]).await;
        let hint = crate::sign_in::HINT.to_owned();
        let turn = [
            Event::Failed(Failure::NotSignedIn { hint }),
            Event::TurnEnded,
        ];
        assert_eq!(started(&events), turn);
    }

    #[tokio::test]
    async fn keeps_only_the_end_of_what_the_binary_says_on_stderr() {
        let said = format!("{}the end", "x".repeat(200_000));
        let kept = tail(&mut said.as_bytes()).await;
        assert_eq!(kept.len(), STDERR_TAIL);
        assert!(kept.ends_with("xthe end"));
    }

    #[tokio::test]
    async fn tells_a_binary_that_ended_on_its_own() {
        let replay = replay(TURN, &["one"], true).await;
        assert!(!replay.closed);
    }
}
