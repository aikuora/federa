use engine::{Event, Failure};
use serde_json::Value;

use crate::sign_in;

/// The events one stdout line carries; a line of a kind the adapter does not use carries none.
pub(crate) fn read(line: &str) -> Result<Vec<Event>, Failure> {
    let message: Value = serde_json::from_str(line)
        .map_err(|error| Failure::Unreadable(format!("a line that is not JSON: {error}")))?;
    match text(&message, "/type")? {
        "system" if message["subtype"] == "init" => Ok(vec![Event::Started {
            session: text(&message, "/session_id")?.to_owned(),
        }]),
        "assistant" => assistant(&message),
        "result" => result(&message),
        _ => Ok(Vec::new()),
    }
}

fn assistant(message: &Value) -> Result<Vec<Event>, Failure> {
    if let Some(error) = message["error"].as_str() {
        return Ok(vec![Event::Failed(failure(error))]);
    }
    let content = message
        .pointer("/message/content")
        .and_then(Value::as_array)
        .ok_or_else(|| missing("assistant", "/message/content"))?;
    let mut events = Vec::new();
    for block in content {
        match text(block, "/type")? {
            "text" => events.push(Event::Text(text(block, "/text")?.to_owned())),
            "tool_use" => events.push(Event::ToolCalled {
                name: text(block, "/name")?.to_owned(),
            }),
            _ => {}
        }
    }
    Ok(events)
}

// Every turn ends with its result line. A failed API call, such as a session not signed in, is
// told by the assistant line's `error` and still ends with a `success` result, so only the other
// subtypes, such as running out of turns, are failures of their own.
fn result(message: &Value) -> Result<Vec<Event>, Failure> {
    if text(message, "/subtype")? == "success" {
        return Ok(vec![Event::TurnEnded]);
    }
    let errors: Vec<&str> = message["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let reason = if errors.is_empty() {
        text(message, "/subtype")?.to_owned()
    } else {
        errors.join("; ")
    };
    Ok(vec![
        Event::Failed(Failure::Engine(reason)),
        Event::TurnEnded,
    ])
}

fn failure(error: &str) -> Failure {
    match error {
        "authentication_failed" => Failure::NotSignedIn {
            hint: sign_in::HINT.to_owned(),
        },
        other => Failure::Engine(other.to_owned()),
    }
}

fn text<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, Failure> {
    let kind = value["type"].as_str().unwrap_or("a line");
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| missing(kind, pointer))
}

fn missing(kind: &str, pointer: &str) -> Failure {
    Failure::Unreadable(format!("{kind} without {pointer}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_kinds_it_does_not_use() {
        let lines = [
            r#"{"type":"user","message":{"content":[]}}"#,
            r#"{"type":"system","subtype":"api_retry"}"#,
            r#"{"type":"rate_limit_event","new_field":1}"#,
        ];
        for line in lines {
            assert_eq!(read(line), Ok(vec![]), "{line}");
        }
    }

    #[test]
    fn names_the_field_it_needs_and_is_missing() {
        let cases = [
            (
                r#"{"type":"system","subtype":"init"}"#,
                "system without /session_id",
            ),
            (
                r#"{"type":"assistant","message":{}}"#,
                "assistant without /message/content",
            ),
            (
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use"}]}}"#,
                "tool_use without /name",
            ),
            (
                r#"{"type":"result","is_error":false}"#,
                "result without /subtype",
            ),
            (r#"{"subtype":"init"}"#, "a line without /type"),
        ];
        for (line, reason) in cases {
            assert_eq!(
                read(line),
                Err(Failure::Unreadable(reason.into())),
                "{line}"
            );
        }
    }

    #[test]
    fn says_a_line_is_not_json() {
        let Err(Failure::Unreadable(reason)) = read("Not logged in") else {
            panic!("a line that is not JSON was read");
        };
        assert!(reason.starts_with("a line that is not JSON"), "{reason}");
    }

    #[test]
    fn ends_a_turn_that_failed_on_its_own_after_reporting_why() {
        let line =
            r#"{"type":"result","subtype":"error_max_turns","is_error":true,"errors":["a","b"]}"#;
        let failure = Event::Failed(Failure::Engine("a; b".into()));
        assert_eq!(read(line), Ok(vec![failure, Event::TurnEnded]));
        let line = r#"{"type":"result","subtype":"error_during_execution","is_error":true}"#;
        let failure = Event::Failed(Failure::Engine("error_during_execution".into()));
        assert_eq!(read(line), Ok(vec![failure, Event::TurnEnded]));
    }

    #[test]
    fn turns_any_other_assistant_error_into_a_failure_of_the_engine() {
        let line = r#"{"type":"assistant","error":"rate_limit","message":{"content":[]}}"#;
        let failure = Failure::Engine("rate_limit".into());
        assert_eq!(read(line), Ok(vec![Event::Failed(failure)]));
    }
}
