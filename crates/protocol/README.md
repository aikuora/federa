# protocol

The contract between the runtime and its clients, such as the desktop app or a TUI. A client depends
on this crate, or on the TypeScript types generated from it, and on nothing else of the core.

## Scope

### Responsibilities

- **client-contract**: The commands a client sends to the runtime, the replies and events the runtime
  answers with, and the greeting where both declare the protocol version.
- **typescript-types**: The TypeScript types of the contract, generated from the Rust types so a
  client and the runtime cannot drift apart.

### Out of scope

- The transport that carries the frames, which `apps/runtime` serves.
- What the runtime does with each command, which `kernel` decides.

### Design guidelines

- A new capability of a client is a new `Command` with its `Reply` and, when the runtime answers in
  parts or on its own, its `Event`. The types carry no logic.

## Public interface

- `ClientFrame` and `RuntimeFrame`: everything that crosses a connection, in each direction.
- `Hello` and `VERSION`: the greeting and the version it declares.
- `Command`, `Reply`, `Event`, `Outcome` and `Failure`: what a client asks, what it gets back and why
  something failed.
- `Organization`, `Asset`, `Conversation` and the types they hold: the domain as a client sees it.
- The private pnpm package `protocol`: each type above as `protocol/<Type>`, generated in
  `bindings/`, and `VERSION` as `protocol/version`.

Its compatibility starts to hold with the first published version of the app, which has not come
yet.

## Development

### Testing

`cargo test -p protocol` checks the JSON of the frames, and fails when `bindings/` no longer holds
the TypeScript types the Rust ones generate. After changing a type, regenerate them with
`BINDINGS=update cargo test -p protocol --test bindings` and commit `bindings/` with the change.
