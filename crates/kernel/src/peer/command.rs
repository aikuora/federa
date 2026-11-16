use std::sync::{Arc, MutexGuard, atomic::Ordering};

use engine::McpServer;
use federation::Federation;
use protocol::{AssetName, Conversation, Failure, FailureCode, Reply, TurnId};

use crate::{
    Kernel,
    agent::{AgentIdentity, Role},
    asset, dismiss, failure, opened, peer,
};

impl Kernel {
    /// Opens the asset's peer if it is not running, answers with the turn of its answer, and plays
    /// that turn after the reply, storing both messages and the engine's session.
    pub(crate) async fn send(&self, asset: AssetName, text: String) -> Result<Reply, Failure> {
        let name = asset::parse(&asset)?;
        let (agent, organization) = {
            let open = self.open();
            let agent = self.peer(&open, &name)?;
            (agent, opened(&open)?.root().to_owned())
        };
        let turn = TurnId(self.turns.fetch_add(1, Ordering::Relaxed));
        let (peers, agents) = (self.peers.clone(), self.agents.clone());
        let (conversations, events) = (self.conversations.clone(), self.events.clone());
        tokio::spawn(async move {
            let played = peer::play(
                &agent,
                &conversations,
                &organization,
                &name,
                turn,
                text,
                &events,
            )
            .await;
            if played.is_err()
                && let Some(token) = peers.close(&name, &agent)
            {
                dismiss(&agents, &token);
            }
            peer::announce(&events, &name, turn, played);
        });
        Ok(Reply::MessageAccepted { turn })
    }

    pub(crate) fn load(&self, asset: &AssetName) -> Result<Reply, Failure> {
        let name = asset::parse(asset)?;
        let open = self.open();
        let federation = opened(&open)?;
        federation.asset(&name).map_err(failure::from_federation)?;
        let stored = self.conversations.load(federation.root(), &name)?;
        Ok(Reply::Conversation {
            conversation: Conversation {
                asset: asset.clone(),
                messages: stored.messages,
            },
        })
    }

    /// The asset's running peer, opened if there is none.
    fn peer(
        &self,
        open: &MutexGuard<'_, Option<Federation>>,
        asset: &federation::AssetName,
    ) -> Result<Arc<tokio::sync::Mutex<engine::Agent>>, Failure> {
        if let Some(agent) = self.peers.get(asset) {
            return Ok(agent);
        }
        let federation = opened(open)?;
        let engine = self.engine.engine().map_err(|reason| Failure {
            code: FailureCode::Internal,
            message: reason.to_owned(),
        })?;
        let session = self.conversations.load(federation.root(), asset)?.session;
        let identity = AgentIdentity {
            role: Role::Peer,
            asset: asset.clone(),
        };
        let token = self.admit(open, identity);
        let tools = McpServer {
            name: "federa".to_owned(),
            url: self.tools.clone(),
            token: token.as_str().to_owned(),
        };
        match peer::open(engine.as_ref(), federation, asset, tools, session) {
            Ok(agent) => Ok(self.peers.insert(asset.clone(), agent, token)),
            Err(failure) => {
                dismiss(&self.agents, &token);
                Err(failure)
            }
        }
    }
}
