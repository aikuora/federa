# kernel

The core's logic. It answers every command of the protocol for the person who runs it: it opens
organizations, registers their assets and, later, runs the agents that work on them. It knows the
engine contract and the federation repository, and never a concrete engine.

## Scope

### Responsibilities

- **organization**: Creating and opening the organization the runtime works on, one at a time.
- **asset**: Registering the managed assets of the open organization and listing them.
- **agent-tools**: The tools agents call, each call checked against the identity of the agent that
  makes it, which the kernel gave it when the agent opened.

### Out of scope

- How anything is stored on disk, which `federation` does.
- Serving clients over a transport, which `apps/runtime` does.
- Any concrete engine; agents run through `engine`.

### Design guidelines

- Every capability a client reaches starts as a protocol `Command` that `Kernel::handle` answers,
  in the module of its concept.

## Public interface

- `Kernel`: built with the person who uses the runtime; answers each `protocol::Command` with its
  `protocol::Outcome`, admits agents and runs the tools they call.
- `AgentIdentity`, `Role` and `AgentToken`: who an agent is, and the token it calls tools with.
- `Tool` and `ToolError`: a tool as an MCP server describes it, and why a call failed.

## Development

### Testing

- The tests in `tests/` drive the kernel only through its public interface, `Kernel::handle` and the
  agent tools, on federation repositories and asset repositories in temporary folders, and need the
  `git` binary.
