use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, PoisonError},
};

use engine::{AgentSpec, Engine, McpServer};
use federation::{AssetName, Federation, read_asset};
use protocol::{Author, Event, Failure, Message, TurnId};
use tokio::sync::{Mutex, broadcast};

use crate::{AgentToken, asset, conversation::Conversations, failure};

mod command;
mod instructions;

// One peer runs per asset, opened at the first message about it and kept while its organization
// stays open, so it keeps what was said; its turns run one at a time. A turn's parts go out as
// events after the reply that named the turn, because the kernel answers before the turn's task
// reads the agent. A turn that fails closes its peer, and the next message opens it again on the
// session stored so far.

/// The peers running for the open organization.
#[derive(Default)]
pub(crate) struct Peers {
    running: std::sync::Mutex<HashMap<AssetName, Running>>,
}

struct Running {
    agent: Arc<Mutex<engine::Agent>>,
    /// The token the peer calls the kernel's tools with, dismissed when it closes.
    token: AgentToken,
}

impl Peers {
    /// The asset's running peer, if there is one.
    pub(crate) fn get(&self, asset: &AssetName) -> Option<Arc<Mutex<engine::Agent>>> {
        let running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        running.get(asset).map(|peer| peer.agent.clone())
    }

    pub(crate) fn insert(
        &self,
        asset: AssetName,
        agent: engine::Agent,
        token: AgentToken,
    ) -> Arc<Mutex<engine::Agent>> {
        let agent = Arc::new(Mutex::new(agent));
        let peer = Running {
            agent: agent.clone(),
            token,
        };
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        running.insert(asset, peer);
        agent
    }

    /// Closes the asset's peer if `agent` is still the one running, and returns the token to
    /// dismiss; a peer opened since, or one closed with its organization, stays as it is.
    pub(crate) fn close(
        &self,
        asset: &AssetName,
        agent: &Arc<Mutex<engine::Agent>>,
    ) -> Option<AgentToken> {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if !running
            .get(asset)
            .is_some_and(|peer| Arc::ptr_eq(&peer.agent, agent))
        {
            return None;
        }
        running.remove(asset).map(|peer| peer.token)
    }

    /// Closes every peer, as the organization they belong to closes.
    pub(crate) fn close_all(&self) {
        // Dropping an agent closes its input, so it finishes and ends; a turn still playing holds
        // it until that turn ends.
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

/// Opens the peer of `asset` on `engine`, in the asset's folder, reaching the kernel's tools at
/// `tools` and continuing `resume`, the session stored for it, if there is one.
pub(crate) fn open(
    engine: &dyn Engine,
    federation: &Federation,
    asset: &AssetName,
    tools: McpServer,
    resume: Option<String>,
) -> Result<engine::Agent, Failure> {
    let read = |entry| {
        let workdir = asset::folder(&entry);
        let knowledge = read_asset(&workdir)?.knowledge;
        Ok((workdir, knowledge, federation.knowledge()?))
    };
    let (workdir, knowledge, organization) = federation
        .asset(asset)
        .and_then(read)
        .map_err(failure::from_federation)?;
    let spec = AgentSpec {
        instructions: instructions::peer(asset, &organization, &knowledge),
        workdir,
        mcp: vec![tools],
        resume,
    };
    engine.open(spec).map_err(failure::from_engine_error)
}

/// Plays the person's `text` to the peer of `asset` as `turn`, and stores both messages and the
/// engine's session in the conversation; how the turn ended is announced apart, once the peer of a
/// failed turn is closed, so a client that hears it never reaches that peer again.
pub(crate) async fn play(
    agent: &Mutex<engine::Agent>,
    conversations: &Conversations,
    organization: &Path,
    asset: &AssetName,
    turn: TurnId,
    text: String,
    events: &broadcast::Sender<Event>,
) -> Result<(), Failure> {
    // Held from loading the conversation to saving it, so turns of one asset never overwrite each
    // other's messages.
    let mut agent = agent.lock().await;
    let mut stored = conversations.load(organization, asset)?;
    stored.messages.push(Message {
        author: Author::Person,
        text: text.clone(),
    });
    let answer = self::turn(&mut agent, asset, turn, text, events, &mut stored.session).await;
    if let Ok(answer) = &answer {
        stored.messages.push(Message {
            author: Author::Peer,
            text: answer.clone(),
        });
    }
    // Saved even when the turn failed, so the person's message and the session are kept.
    let saved = conversations.save(organization, asset, &stored);
    answer.and(saved)
}

/// Announces how `turn` ended.
pub(crate) fn announce(
    events: &broadcast::Sender<Event>,
    asset: &AssetName,
    turn: TurnId,
    played: Result<(), Failure>,
) {
    let asset = protocol::AssetName(asset.to_string());
    let event = match played {
        Ok(()) => Event::PeerDone { asset, turn },
        Err(failure) => Event::PeerFailed {
            asset,
            turn,
            failure,
        },
    };
    // Sending fails only when no client is connected, and then nobody is left to tell.
    let _ = events.send(event);
}

/// Plays one turn: sends `text` to the peer and announces each part of its answer on `events`
/// until the turn ends, and returns the answer joined, or why the turn failed. The session the
/// engine reports, if it reports one, goes to `session`.
pub(crate) async fn turn(
    agent: &mut engine::Agent,
    asset: &AssetName,
    turn: TurnId,
    text: String,
    events: &broadcast::Sender<Event>,
    session: &mut Option<String>,
) -> Result<String, Failure> {
    agent.send(text).await.map_err(failure::from_engine_error)?;
    let mut parts = Vec::new();
    let mut failed = None;
    while let Some(event) = agent.next().await {
        match event {
            engine::Event::Started { session: started } => *session = Some(started),
            engine::Event::Text(text) => {
                let part = Event::PeerPart {
                    asset: protocol::AssetName(asset.to_string()),
                    turn,
                    text: text.clone(),
                };
                let _ = events.send(part);
                parts.push(text);
            }
            engine::Event::ToolCalled { .. } => {}
            engine::Event::Failed(failure) => {
                failed.get_or_insert(failure);
            }
            engine::Event::TurnEnded => {
                return match failed {
                    None => Ok(parts.join("\n\n")),
                    Some(failure) => Err(failure::from_engine(failure)),
                };
            }
        }
    }
    let ended = engine::Failure::Engine("the peer ended in the middle of its answer".to_owned());
    Err(failure::from_engine(failed.unwrap_or(ended)))
}
