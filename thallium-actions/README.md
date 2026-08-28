# thallium-actions

The sole semantic workflow coordinator for the Thallium 81 backend: it validates commands, applies pure state reductions, invokes VFS/scheduler/watcher effects, and feeds owned results back into state.

## Boundary invariants

- This is the only layer that coordinates state reduction with asynchronous effects; services never mutate browser state.
- Mutable browser-state access never crosses an `.await`. Validation, reduction, and feedback are synchronous phases; asynchronous effects consume and return owned values.
- Reducers remain pure and never perform I/O, render, prompt, launch processes, or call Qt.
- Commands, actions, effects, and feedback are typed and owned. They contain no Qt/render types, Lua `Scope`, callbacks, reply senders, `Data::Any`, or borrowed runtime objects.
- Every filesystem operation uses an explicit backend-qualified location; this crate never changes process-global CWD.
- Runtime handles and Tokio synchronization remain internal implementation details, not protocol or state values.

The boundary comes from `PHASE2-PLAN.md` sections 1, 4, and 5. The split-phase coordination shape is informed by `WATCHER-STATE-RESEARCH.md` sections 2 and 3.
