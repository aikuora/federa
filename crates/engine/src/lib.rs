//! The contract between the kernel and the engines that run agents, one adapter per provider. The
//! kernel opens, talks to and closes agents through it, and never knows which engine runs them.

mod agent;
mod error;
mod event;
mod spec;

pub use agent::Agent;
pub use error::Error;
pub use event::{Event, Failure};
pub use spec::{AgentSpec, McpServer};

/// An engine able to run agents, such as the official binary of one provider.
pub trait Engine: Send + Sync {
    /// Starts an agent. It is ready for its first message when this returns.
    fn open(&self, spec: AgentSpec) -> Result<Agent, Error>;
}
