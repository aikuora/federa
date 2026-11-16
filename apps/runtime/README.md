# runtime

The process that holds one person's core, apart from any window, so agents keep working when the
app closes. It wires the core's crates together and serves the protocol to clients, on the person's
machine or on a server.

## Scope

### Responsibilities

- **process**: Running one person's core as a process of its own, which clients connect to and
  leave.
- **client-server**: Serving the protocol to clients over WebSocket, only on `127.0.0.1` and only to
  a client that presents a token only the person can read.
- **data-folder**: The runtime's own folder on the machine: where clients find a running runtime,
  and the configuration of Claude the app's agents run with.
- **agent-endpoint**: Serving the kernel's tools to agents over MCP on HTTP, only to an agent that
  presents its own token.

### Out of scope

- What each command does, which `kernel` decides.
- How the federation repository is stored, which `federation` hides.

### Design guidelines

- The runtime wires crates and serves them; logic of its own goes to `kernel`.

## Public interface

- The `runtime` binary, which serves until it is stopped.
- `runtime.toml` in the data folder: `schema = 1`, the `port` and the `token`, readable only by its
  owner. A client reads it to find the runtime.
- The WebSocket at `ws://127.0.0.1:<port>/`, which takes the token as `Authorization: Bearer
  <token>` and then speaks the `protocol` crate's frames as JSON text messages.
- The MCP endpoint at `http://127.0.0.1:<port>/mcp`, which takes an agent's token as
  `Authorization: Bearer <token>` and answers `initialize`, `tools/list` and `tools/call` over
  MCP's Streamable HTTP transport, each with one JSON body and no session.
- The data folder: `FEDERA_DATA_DIR` when set, otherwise `federa` inside the platform's data folder
  as the `dirs` crate's `data_dir` gives it (`~/Library/Application Support/federa` on macOS,
  `$XDG_DATA_HOME/federa` or `~/.local/share/federa` on Linux, `%APPDATA%\federa` on Windows). A
  client computes the same path to find `runtime.toml`.
- The person's identity: `user.email` in git's global configuration or a file it includes, read
  from the data folder so an `includeIf` does not depend on where the runtime starts. Without it the
  runtime exits with an error that says how to set it.

## Development

### Build and run

- `FEDERA_DATA_DIR=<scratch folder> cargo run -p runtime`. Without `FEDERA_DATA_DIR` it publishes
  the machine's real `runtime.toml`, the one the desktop app reads.

### Testing

- The tests in `tests/` run the real binary with a temporary `FEDERA_DATA_DIR` and a git
  configuration of their own (`GIT_CONFIG_GLOBAL`), read its `runtime.toml` and connect as a
  client. They need the `git` binary.
- The tests in `src/server.rs` drive the served routes in-process, admitting agents in the kernel
  directly, which a test outside the process cannot do. They need the `git` binary too.
