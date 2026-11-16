use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{Asset, AssetName, Conversation, Failure, Organization, SignInStatus, TurnId};

/// The greeting each side sends first on a connection, declaring the protocol version it speaks.
/// When the versions differ, the client warns the person and the runtime closes the connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Hello {
    pub protocol: u32,
}

/// Chosen by the client for each command, and repeated in its reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct RequestId(pub u32);

/// What a client sends. The first frame of a connection is always `Hello`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum ClientFrame {
    Hello(Hello),
    Command { id: RequestId, command: Command },
}

/// What the runtime sends. Every command gets exactly one `Reply`; events arrive on their own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum RuntimeFrame {
    Hello(Hello),
    Reply { id: RequestId, outcome: Outcome },
    Event { event: Event },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Done { reply: Reply },
    Failed { failure: Failure },
}

/// What a client asks. The commands that need an organization act on the one the runtime has
/// open, since there is one at a time. `create_organization` turns a new, empty folder into the
/// federation repository of a new organization and opens it, and `register_asset` takes the folder
/// that holds the asset's `.federa/`. `send_message` is answered with `message_accepted` and the
/// turn of the peer's answer, which then arrives as the `peer_*` events of that turn.
/// `start_sign_in` starts the engine's official sign-in and answers `signing_in`, or `unavailable`
/// with the reason; every later change of the status arrives as `sign_in_changed`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    CreateOrganization { folder: PathBuf, name: String },
    OpenOrganization { folder: PathBuf },
    RegisterAsset { folder: PathBuf },
    ListAssets,
    SendMessage { asset: AssetName, text: String },
    LoadConversation { asset: AssetName },
    SignInStatus,
    StartSignIn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "reply", rename_all = "snake_case")]
pub enum Reply {
    Organization { organization: Organization },
    Asset { asset: Asset },
    Assets { assets: Vec<Asset> },
    MessageAccepted { turn: TurnId },
    Conversation { conversation: Conversation },
    SignIn { status: SignInStatus },
}

/// The peer's answer, which arrives after the `message_accepted` reply that named its turn, so a
/// client always knows the turn before its parts. The `peer_part` events of a turn come in order,
/// and the turn ends with one `peer_done`, whose parts joined are the stored message, or one
/// `peer_failed`. `sign_in_changed` carries the engine's new sign-in status whenever it changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    PeerPart {
        asset: AssetName,
        turn: TurnId,
        text: String,
    },
    PeerDone {
        asset: AssetName,
        turn: TurnId,
    },
    PeerFailed {
        asset: AssetName,
        turn: TurnId,
        failure: Failure,
    },
    SignInChanged {
        status: SignInStatus,
    },
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use serde::de::DeserializeOwned;
    use serde_json::{Value, json};

    use super::*;
    use crate::{Author, FailureCode, Message, Stage};

    fn assert_wire<T: Serialize + DeserializeOwned + PartialEq + Debug>(frame: T, wire: Value) {
        assert_eq!(serde_json::to_value(&frame).unwrap(), wire);
        assert_eq!(serde_json::from_value::<T>(wire).unwrap(), frame);
    }

    fn command(id: u32, command: Command) -> ClientFrame {
        ClientFrame::Command {
            id: RequestId(id),
            command,
        }
    }

    fn done(id: u32, reply: Reply) -> RuntimeFrame {
        RuntimeFrame::Reply {
            id: RequestId(id),
            outcome: Outcome::Done { reply },
        }
    }

    fn billing() -> AssetName {
        AssetName("billing".into())
    }

    #[test]
    fn hello_carries_the_version_beside_its_tag() {
        let wire = json!({ "frame": "hello", "protocol": 1 });
        assert_wire(ClientFrame::Hello(Hello { protocol: 1 }), wire.clone());
        assert_wire(RuntimeFrame::Hello(Hello { protocol: 1 }), wire);
    }

    #[test]
    fn commands_nest_inside_their_frame() {
        assert_wire(
            command(1, Command::ListAssets),
            json!({ "frame": "command", "id": 1, "command": { "command": "list_assets" } }),
        );
        assert_wire(
            command(
                2,
                Command::CreateOrganization {
                    folder: "/work/acme".into(),
                    name: "Acme".into(),
                },
            ),
            json!({
                "frame": "command",
                "id": 2,
                "command": { "command": "create_organization", "folder": "/work/acme", "name": "Acme" },
            }),
        );
        assert_wire(
            command(
                3,
                Command::SendMessage {
                    asset: billing(),
                    text: "What does it do?".into(),
                },
            ),
            json!({
                "frame": "command",
                "id": 3,
                "command": { "command": "send_message", "asset": "billing", "text": "What does it do?" },
            }),
        );
    }

    #[test]
    fn replies_carry_their_outcome() {
        assert_wire(
            done(
                4,
                Reply::Assets {
                    assets: vec![Asset {
                        name: billing(),
                        stage: Stage::Production,
                        managed: true,
                    }],
                },
            ),
            json!({
                "frame": "reply",
                "id": 4,
                "outcome": {
                    "outcome": "done",
                    "reply": {
                        "reply": "assets",
                        "assets": [{ "name": "billing", "stage": "production", "managed": true }],
                    },
                },
            }),
        );
        assert_wire(
            done(
                5,
                Reply::Conversation {
                    conversation: Conversation {
                        asset: billing(),
                        messages: vec![Message {
                            author: Author::Peer,
                            text: "It charges customers.".into(),
                        }],
                    },
                },
            ),
            json!({
                "frame": "reply",
                "id": 5,
                "outcome": {
                    "outcome": "done",
                    "reply": {
                        "reply": "conversation",
                        "conversation": {
                            "asset": "billing",
                            "messages": [{ "author": "peer", "text": "It charges customers." }],
                        },
                    },
                },
            }),
        );
        assert_wire(
            RuntimeFrame::Reply {
                id: RequestId(6),
                outcome: Outcome::Failed {
                    failure: Failure {
                        code: FailureCode::NoOrganization,
                        message: "Open an organization first.".into(),
                    },
                },
            },
            json!({
                "frame": "reply",
                "id": 6,
                "outcome": {
                    "outcome": "failed",
                    "failure": { "code": "no_organization", "message": "Open an organization first." },
                },
            }),
        );
    }

    #[test]
    fn events_nest_inside_their_frame() {
        assert_wire(
            RuntimeFrame::Event {
                event: Event::PeerPart {
                    asset: billing(),
                    turn: TurnId(7),
                    text: "It charges".into(),
                },
            },
            json!({
                "frame": "event",
                "event": { "event": "peer_part", "asset": "billing", "turn": 7, "text": "It charges" },
            }),
        );
        assert_wire(
            RuntimeFrame::Event {
                event: Event::PeerDone {
                    asset: billing(),
                    turn: TurnId(7),
                },
            },
            json!({ "frame": "event", "event": { "event": "peer_done", "asset": "billing", "turn": 7 } }),
        );
    }

    #[test]
    fn an_unknown_command_is_refused() {
        let wire =
            json!({ "frame": "command", "id": 8, "command": { "command": "delete_assets" } });
        assert!(serde_json::from_value::<ClientFrame>(wire).is_err());
    }
}
