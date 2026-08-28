# thallium-protocol

Owned, data-only values presented by the Thallium backend to its future
cxx-qt/QML adapter.

This crate is deliberately narrower than a daemon wire protocol. It contains
state snapshots/deltas, task information, stable scalar IDs, and filesystem
DTO wrappers that can later be mapped into `#[cxx_qt::bridge]` QObject and Qt
model types. It depends only on owned value types from `thallium-fs-core`.

Boundary invariants:

- no Qt, QML, renderer, terminal, or geometry types;
- no Tokio runtime, channel, `JoinHandle`, or cancellation type;
- no `Arc`, `Mutex`, atomics, borrowed handles, callbacks, closures, or VFS
  streams;
- every published collection and string is owned;
- filesystem identity remains byte-preserving rather than forced through
  UTF-8; display strings are separate presentation data; and
- mutable registries and browser state remain behind the backend boundary.

The constraints and initial DTO vocabulary come from `PHASE2-PLAN.md` section
5, with task and state shapes informed by `SCHEDULER-RESEARCH.md` and
`WATCHER-STATE-RESEARCH.md`.
