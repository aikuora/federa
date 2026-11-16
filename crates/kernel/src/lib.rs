//! The core's logic: what each protocol command does to an organization, its assets and their
//! agents. The runtime hands it every command and sends back what it answers, and it admits each
//! agent and answers the tool calls the agent makes; how anything is stored stays in `federation`.

mod agent;
mod asset;
mod conversation;
mod failure;
mod organization;
mod peer;
mod sign_in;
mod tools;

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard, PoisonError, atomic::AtomicU32},
};

use engine::Engine;

use federation::Federation;
use protocol::{Command, Event, Failure, FailureCode, Outcome, Reply};
use serde_json::Value;
use tokio::sync::broadcast;

use agent::{AgentIdentity, AgentToken};
pub use tools::{Tool, ToolError};

type Agents = Arc<Mutex<HashMap<String, AgentIdentity>>>;

// One organization is open at a time, as the protocol says; opening another replaces it, closes its
// peers and dismisses every agent, so a token admitted under one organization never reads another's
// asset of the same name. Opening the organization already open keeps it, with its peers running,
// since a client opens it again whenever it starts.

/// The core of one person's runtime.
pub struct Kernel {
    /// Who uses this runtime. They own every asset they register while they work alone.
    person: String,
    organization: Mutex<Option<Federation>>,
    /// Shared with the turns, which dismiss their peer's token when they close it.
    agents: Agents,
    engine: sign_in::Slot,
    /// What the kernel announces on its own; every connected client gets it.
    events: broadcast::Sender<Event>,
    peers: Arc<peer::Peers>,
    conversations: Arc<conversation::Conversations>,
    /// Where agents reach the kernel's tools over MCP.
    tools: String,
    /// The id of the next turn of any peer.
    turns: AtomicU32,
}

impl Kernel {
    /// `engine` is the engine the agents run on, or why the runtime cannot run it; the kernel
    /// still answers every command that needs no agent without one. `conversations` is the folder,
    /// private to the person, where conversations are kept, and `tools` the URL of the MCP endpoint
    /// that serves the kernel's tools to agents.
    pub fn new(
        person: String,
        engine: Result<Arc<dyn Engine>, String>,
        conversations: PathBuf,
        tools: String,
    ) -> Self {
        Self {
            person,
            organization: Mutex::new(None),
            agents: Agents::default(),
            engine: sign_in::Slot::new(engine),
            events: broadcast::channel(64).0,
            peers: Arc::default(),
            conversations: Arc::new(conversation::Conversations::new(conversations)),
            tools,
            turns: AtomicU32::new(0),
        }
    }

    /// Every event the kernel announces from now on.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.events.subscribe()
    }

    /// Answers one command.
    pub async fn handle(&self, command: Command) -> Outcome {
        match self.answer(command).await {
            Ok(reply) => Outcome::Done { reply },
            Err(failure) => Outcome::Failed { failure },
        }
    }

    /// Admits an agent, and returns the token it presents on every tool call. Takes the
    /// organization locked, as `call` and `replace` lock it before the agents, so an agent is never
    /// admitted while the organization is being replaced.
    fn admit(
        &self,
        _open: &MutexGuard<'_, Option<Federation>>,
        agent: AgentIdentity,
    ) -> AgentToken {
        let token = AgentToken::generate();
        self.agents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(token.as_str().to_owned(), agent);
        token
    }

    /// The tools the agent holding `token` may call.
    pub fn tools(&self, token: &str) -> Result<Vec<Tool>, ToolError> {
        Ok(tools::available(&self.agent(token)?))
    }

    /// Runs `tool` for the agent holding `token`, and returns its text answer.
    pub async fn call(&self, token: &str, tool: &str, input: Value) -> Result<String, ToolError> {
        // The organization is locked first, so it cannot be replaced, dismissing the agent, between
        // finding the agent and running its tool.
        let open = self.open();
        let agent = self.agent(token)?;
        let federation = open
            .as_ref()
            .ok_or_else(|| ToolError::Failed("No organization is open.".to_owned()))?;
        tools::run(federation, &agent, tool, &input)
    }

    // A map lookup, not a constant-time comparison: the map's hashing is keyed at random per
    // process, so how long a lookup takes tells a caller nothing about how close a guess was.
    fn agent(&self, token: &str) -> Result<AgentIdentity, ToolError> {
        let agents = self.agents.lock().unwrap_or_else(PoisonError::into_inner);
        agents.get(token).cloned().ok_or(ToolError::Unauthorized)
    }

    fn replace<'a>(
        &self,
        open: &'a mut Option<Federation>,
        federation: Federation,
    ) -> &'a Federation {
        self.peers.close_all();
        self.agents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        open.insert(federation)
    }

    // The organization is locked only by the commands that use it, so the sign-in, which waits on
    // the engine, never holds it.
    async fn answer(&self, command: Command) -> Result<Reply, Failure> {
        match command {
            Command::CreateOrganization { folder, name } => {
                let mut open = self.open();
                let federation = self.replace(&mut open, organization::create(&folder, &name)?);
                Ok(Reply::Organization {
                    organization: organization::summary(federation),
                })
            }
            Command::OpenOrganization { folder } => {
                let mut open = self.open();
                let federation = organization::open(&folder)?;
                let federation = match open.as_ref() {
                    Some(current) if organization::same(current, &federation) => {
                        open.insert(federation)
                    }
                    _ => self.replace(&mut open, federation),
                };
                Ok(Reply::Organization {
                    organization: organization::summary(federation),
                })
            }
            Command::RegisterAsset { folder } => Ok(Reply::Asset {
                asset: asset::register(opened(&self.open())?, &self.person, &folder)?,
            }),
            Command::ListAssets => Ok(Reply::Assets {
                assets: asset::list(opened(&self.open())?)?,
            }),
            Command::SendMessage { asset, text } => self.send(asset, text).await,
            Command::LoadConversation { asset } => self.load(&asset),
            Command::SignInStatus => Ok(Reply::SignIn {
                status: self.engine.status().await,
            }),
            Command::StartSignIn => Ok(Reply::SignIn {
                status: self.engine.start(self.events.clone()),
            }),
        }
    }

    fn open(&self) -> MutexGuard<'_, Option<Federation>> {
        // The slot holds no invariant a panic could break halfway, so a poisoned lock is still
        // safe to use, and one failed command does not fail every later one.
        self.organization
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

fn opened(open: &Option<Federation>) -> Result<&Federation, Failure> {
    open.as_ref().ok_or_else(|| Failure {
        code: FailureCode::NoOrganization,
        message: "Open an organization first.".to_owned(),
    })
}

fn dismiss(agents: &Agents, token: &AgentToken) {
    let mut agents = agents.lock().unwrap_or_else(PoisonError::into_inner);
    agents.remove(token.as_str());
}
