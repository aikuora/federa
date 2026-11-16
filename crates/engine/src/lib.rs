//! The contract between the kernel and the engines that run agents, one adapter per provider. The
//! kernel opens, talks to and closes agents through it, and never knows which engine runs them.

mod agent;
mod error;
mod event;
mod sign_in;
mod spec;

pub use agent::Agent;
pub use error::Error;
pub use event::{Event, Failure};
pub use sign_in::SignIn;
pub use spec::{AgentSpec, McpServer};

/// An engine able to run agents, such as the official binary of one provider.
pub trait Engine: Send + Sync {
    /// Starts an agent. It is ready for its first message when this returns.
    fn open(&self, spec: AgentSpec) -> Result<Agent, Error>;

    /// Whether the person is signed in to the provider, where this engine's agents run.
    fn sign_in_status(&self) -> Result<SignIn, Error>;

    /// Starts the provider's official sign-in, which the person completes in the browser,
    /// replacing one still running.
    fn start_sign_in(&self) -> Result<(), Error>;

    /// Ends a sign-in still running, if there is one.
    fn stop_sign_in(&self);
}
