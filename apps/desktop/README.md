# desktop

The desktop app: the visual layer where the person works with their organization. A web interface
in React draws it, and a thin native side in Tauri finds or starts the person's runtime and carries
the protocol between the two.

## Scope

### Responsibilities

- **runtime-connection**: Finding the person's running runtime, starting it when none answers, and
  holding the connection to it on the native side, so its token never reaches the web view.
- **organization-setup**: Creating an organization in a new folder, or opening one, from the app.
- **asset-map**: Showing the open organization's assets and registering a new one from its folder.
- **asset-conversation**: Talking with the peer inside an asset, with the conversation kept there.
- **claude-connection**: Showing whether Claude is connected, and starting its official sign-in.

### Out of scope

- Any logic of the core, which the runtime serves through `protocol`; the app knows only that
  crate.
- Packaging the runtime inside the installer.

### Design guidelines

- Modules are split by feature, each one a folder under `src/`; the interface reaches the runtime
  only through `src/runtime`.
- The native side relays frames without reading their meaning, so a new command needs no change
  there.

## Public interface

- The app itself; it reads `runtime.toml` from the runtime's data folder, by the rule the runtime's
  README gives, and honors `FEDERA_DATA_DIR` the same way. When no runtime answers there, it starts
  the `runtime` binary beside its own executable with `FEDERA_DATA_DIR` set to that folder, and
  leaves it running when the window closes.
- The native side's two commands, which only `src/runtime` calls: `connect`, which reaches the
  runtime and hands each frame it sends to a channel, and `null` once the connection ends; and
  `send`, which carries one frame to it.
- The interface remembers in the web view's storage the folder of the last organization it opened,
  and opens it again on start.

## Development

### Build and run

- `cargo build -p runtime`, then `pnpm --filter desktop tauri dev` with `FEDERA_DATA_DIR` on a
  scratch folder. The app starts the runtime it finds beside itself in `target/debug`; without
  `FEDERA_DATA_DIR` it uses the person's real data folder. The interface is served on port 1420.
- Building on Linux needs the system libraries in Tauri's prerequisites, which CI installs.

### Testing

- `cargo test -p desktop` checks how the native side finds or starts the runtime, with fake
  runtimes in temporary folders, and the relay against a WebSocket server of the test's own; it
  never starts a real runtime.
- The tests of `src/runtime` and of the conversation drive them against a fake of the native side,
  through Vitest.
- `src/journey.test.tsx` walks the goal's journey through the real interface, in a DOM without a
  window, against a real runtime that runs the fake `claude` of `crates/claude/fake`, on folders it
  creates in the system's temporary folder and removes. A bridge in Node stands in for the native
  side. `vp test` builds the runtime with `cargo build -p runtime` for it, and it needs `git`,
  `node` and a shell, so it runs only on Unix.
- No test opens a window.
