# claude

The engine that runs agents on the official `claude` binary, speaking stream-json over stdio, so
each person uses their own subscription through the provider's own sign-in.

## Scope

### Responsibilities

- **claude-adapter**: Running agents on the official `claude` binary over stream-json, with its
  sandbox always on.
- **version-floor**: Requiring a minimum version of the binary and accepting newer ones without
  notice.
- **sign-in**: Checking whether the agents' own configuration folder is signed in and starting the
  official sign-in there.

### Out of scope

- Installing or updating `claude`; it uses the binary the person has.
- Reading, storing or passing on credentials, ever.

### Design guidelines

- Read only the fields it uses, ignore the rest, and fail with a clear error when one it needs is
  missing.
- Tests without network replay real recordings of the minimum version, recorded again when it rises.

## Public interface

- `Claude`: the `engine::Engine` for the official binary, built from the binary's path and the
  configuration folder its agents run on.

## Development

### Testing

- `cargo test -p claude` runs without network. It replays the recordings in `recordings/` and runs
  shell stand-ins for the binary.
- The recordings are the stdout of the minimum version in four scenarios: opening a session, two
  turns, one MCP tool call and a session not signed in. Only the person makes them again, with
  `node crates/claude/recordings/record.ts`. It installs that version into `target/`, and signs in
  once, through the official flow, a configuration folder of its own in the main checkout's
  `target/`, shared by every worktree. Its launch arguments and settings follow `src/launch.rs` and
  `src/config.rs`. It fails if a workdir's Claude files, or a plugin, skill or command from
  anywhere but the binary itself, reach the agent, and reports what the built-in plugins bring.
- `cargo test -p claude --test live -- --ignored` converses two turns with the installed `claude`
  on that folder; only the person runs it.

### Caveats

- `Engine::open` must be called within a Tokio runtime, which runs the agent.
- `Claude::new` rewrites the configuration folder's `settings.json` whole, so nothing synced from
  the person's claude.ai account loads and no AGENTS.md is read; the rest of the folder, the
  sign-in among it, belongs to the binary. The plugins built into the binary still load.
- The binary still binds a socket for messages from the person's other sessions, since only bare
  mode, which takes no subscription, does not; the settings drop every message that arrives on it.
- The binary's sandbox confines its shell commands; its file tools follow its permission rules.
