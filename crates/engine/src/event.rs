/// What an agent reports while it works, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The engine's own id for the agent's session.
    Started { session: String },
    /// The next part of the agent's answer.
    Text(String),
    /// The agent called a tool.
    ToolCalled { name: String },
    /// The agent finished its turn and waits for the next message.
    TurnEnded,
    /// The turn failed; the agent may still take another message.
    Failed(Failure),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    /// The person has not signed in to the provider; `hint` says how, in the provider's own flow.
    #[error("not signed in: {hint}")]
    NotSignedIn { hint: String },
    /// The engine sent something the adapter cannot read, such as a field it needs and is missing.
    #[error("unreadable engine output: {0}")]
    Unreadable(String),
    #[error("{0}")]
    Engine(String),
}
