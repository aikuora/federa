//! The core's logic: what each protocol command does to an organization, its assets and their
//! agents. The runtime hands it every command and sends back what it answers, and it admits each
//! agent and answers the tool calls the agent makes; how anything is stored stays in `federation`.

mod agent;
mod asset;
mod failure;
mod organization;
mod tools;

use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
};

use federation::Federation;
use protocol::{Command, Failure, FailureCode, Outcome, Reply};
use serde_json::Value;

pub use agent::{AgentIdentity, AgentToken, Role};
pub use tools::{Tool, ToolError};

// One organization is open at a time, as the protocol says; opening another replaces it and
// dismisses every agent, so a token admitted under one organization never reads another's asset of
// the same name.

/// The core of one person's runtime.
pub struct Kernel {
    /// Who uses this runtime. They own every asset they register while they work alone.
    person: String,
    organization: Mutex<Option<Federation>>,
    agents: Mutex<HashMap<String, AgentIdentity>>,
}

impl Kernel {
    pub fn new(person: String) -> Self {
        Self {
            person,
            organization: Mutex::new(None),
            agents: Mutex::new(HashMap::new()),
        }
    }

    /// Answers one command. Conversation commands answer `Internal` until the peer exists.
    pub async fn handle(&self, command: Command) -> Outcome {
        match self.answer(command) {
            Ok(reply) => Outcome::Done { reply },
            Err(failure) => Outcome::Failed { failure },
        }
    }

    /// Admits an agent, and returns the token it presents on every tool call.
    pub fn admit(&self, agent: AgentIdentity) -> AgentToken {
        // Locked as `call` and `replace` lock, organization first, so an agent is never admitted
        // while the organization is being replaced.
        let _open = self
            .organization
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
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
        let open = self
            .organization
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
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
        self.agents
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        open.insert(federation)
    }

    fn answer(&self, command: Command) -> Result<Reply, Failure> {
        // The slot holds no invariant a panic could break halfway, so a poisoned lock is still
        // safe to use, and one failed command does not fail every later one.
        let mut open = self
            .organization
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match command {
            Command::CreateOrganization { folder, name } => {
                let federation = self.replace(&mut open, organization::create(&folder, &name)?);
                Ok(Reply::Organization {
                    organization: organization::summary(federation),
                })
            }
            Command::OpenOrganization { folder } => {
                let federation = self.replace(&mut open, organization::open(&folder)?);
                Ok(Reply::Organization {
                    organization: organization::summary(federation),
                })
            }
            Command::RegisterAsset { folder } => Ok(Reply::Asset {
                asset: asset::register(opened(&open)?, &self.person, &folder)?,
            }),
            Command::ListAssets => Ok(Reply::Assets {
                assets: asset::list(opened(&open)?)?,
            }),
            Command::SendMessage { .. } | Command::LoadConversation { .. } => Err(Failure {
                code: FailureCode::Internal,
                message: "Conversations with the peer are not available yet.".to_owned(),
            }),
        }
    }
}

fn opened(open: &Option<Federation>) -> Result<&Federation, Failure> {
    open.as_ref().ok_or_else(|| Failure {
        code: FailureCode::NoOrganization,
        message: "Open an organization first.".to_owned(),
    })
}
