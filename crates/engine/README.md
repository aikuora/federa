# engine

The contract between the kernel and the engines that run agents. Each provider's engine is an
adapter in its own crate that implements it, so the kernel orchestrates agents without knowing which
engine runs them.

## Scope

### Responsibilities

- **agent-contract**: Opening an agent with its instructions, its folder and its MCP servers, sending
  it messages, receiving its events and closing it.

### Out of scope

- Any concrete provider, which lives in its own crate, such as `claude`.
- Deciding which agents run and with what instructions, which `kernel` does.

### Design guidelines

- A new provider is a new crate implementing `Engine`; this crate changes only for something every
  engine shares.

## Public interface

- `Engine`: what an engine adapter implements, opening agents.
- `Agent`: a running agent, to send it messages, read its events and close it. Adapters build it.
- `AgentSpec` and `McpServer`: what an agent is opened with.
- `Event` and `Failure`: what an agent reports while it works.
- `Error`: why an engine could not open an agent or take a message.

## Development

### Testing

- `cargo test -p engine` covers `Agent` against an adapter defined in its tests.
