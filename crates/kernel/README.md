# kernel

The core's logic. It answers every command of the protocol for the person who runs it: it opens
organizations, registers their assets and, later, runs the agents that work on them. It knows the
engine contract and the federation repository, and never a concrete engine.

## Scope

### Responsibilities

- **organization**: Creating and opening the organization the runtime works on, one at a time.
- **asset**: Registering the managed assets of the open organization and listing them.

### Out of scope

- How anything is stored on disk, which `federation` does.
- Serving clients over a transport, which `apps/runtime` does.
- Any concrete engine; agents run through `engine`.

### Design guidelines

- Every capability a client reaches starts as a protocol `Command` that `Kernel::handle` answers,
  in the module of its concept.

## Public interface

- `Kernel`: built with the person who uses the runtime; answers each `protocol::Command` with its
  `protocol::Outcome`.

## Development

### Testing

- The tests in `tests/` drive the kernel only through `Kernel::handle`, on federation repositories
  and asset repositories in temporary folders, and need the `git` binary.
