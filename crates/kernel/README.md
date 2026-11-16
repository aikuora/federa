# kernel

The core's logic. It answers every command of the protocol for the person who runs it: it opens
organizations, registers their assets and runs the agents that work on them. It knows the engine
contract and the federation repository, and never a concrete engine.

## Scope

### Responsibilities

- **organization**: Creating and opening the organization the runtime works on, one at a time.
- **asset**: Registering the managed assets of the open organization and listing them.
- **agent-tools**: The tools agents call, each call checked against the identity of the agent that
  makes it, which the kernel gave it when the agent opened.
- **sign-in**: Telling clients whether the agents' engine is ready, starting its sign-in for the
  person, and announcing when that changes.
- **peer**: Opening each asset's peer for the person, playing their conversation with it, and
  keeping that conversation private to the person's runtime.

### Out of scope

- How the federation repository and each `.federa/` are stored, which `federation` does.
- Serving clients over a transport, which `apps/runtime` does.
- Any concrete engine; agents run through `engine`.

### Design guidelines

- Every capability a client reaches starts as a protocol `Command` that `Kernel::handle` answers,
  in the module of its concept.

## Public interface

- `Kernel`: built with the person who uses the runtime, the engine its agents run on or why there
  is none, the folder where the person's conversations are kept and the URL where agents reach its
  tools over MCP; answers each `protocol::Command` with its `protocol::Outcome`, announces
  `protocol::Event`s to whoever subscribed, and runs the tools its agents call with the token it
  opened them with.
- `Tool` and `ToolError`: a tool as an MCP server describes it, and why a call failed.

## Development

### Testing

- The tests in `tests/` drive the kernel only through its public interface, `Kernel::handle` and the
  agent tools, on federation repositories and asset repositories in temporary folders, and need the
  `git` binary. The tests of the peer, in `tests/peer.rs`, run against an engine defined there,
  which records what each agent opens with, the token among it.
- The tests of the sign-in, in `tests/sign_in.rs`, run against engines defined there, on Tokio's
  paused clock, so the minutes a sign-in may take pass at once.

### Caveats

- `Kernel::handle` must be called within a Tokio runtime, which plays the peers' turns and runs
  their agents.
