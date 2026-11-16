//! The core's logic: what each protocol command does to an organization, its assets and their
//! agents. The runtime hands it every command and sends back what it answers; how anything is
//! stored stays in `federation`.

mod asset;
mod failure;
mod organization;

use std::sync::{Mutex, PoisonError};

use federation::Federation;
use protocol::{Command, Failure, FailureCode, Outcome, Reply};

// One organization is open at a time, as the protocol says; opening another replaces it.

/// The core of one person's runtime.
pub struct Kernel {
    /// Who uses this runtime. They own every asset they register while they work alone.
    person: String,
    organization: Mutex<Option<Federation>>,
}

impl Kernel {
    pub fn new(person: String) -> Self {
        Self {
            person,
            organization: Mutex::new(None),
        }
    }

    /// Answers one command. Conversation commands answer `Internal` until the peer exists.
    pub async fn handle(&self, command: Command) -> Outcome {
        match self.answer(command) {
            Ok(reply) => Outcome::Done { reply },
            Err(failure) => Outcome::Failed { failure },
        }
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
                let federation = open.insert(organization::create(&folder, &name)?);
                Ok(Reply::Organization {
                    organization: organization::summary(federation),
                })
            }
            Command::OpenOrganization { folder } => {
                let federation = open.insert(organization::open(&folder)?);
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
