# thallium-scheduler

Bounded, renderer-neutral scheduling, aggregate task tracking, progress reduction, cancellation, and cleanup for the Thallium 81 backend.

## Boundary invariants

- The crate depends only on filesystem-domain values, the VFS façade, and owned protocol snapshots.
- It never calls Qt, renders UI, invokes frontend callbacks, or mutates browser state.
- Queue and worker communication is bounded and typed; runtime senders, receivers, cancellation tokens, and task join handles remain private implementation details.
- Workers report owned events to a single task-registry reducer instead of locking or mutating the registry directly.
- Task observations cross the public boundary as stable IDs and owned snapshots, never as callbacks, closures, borrowed registry entries, or Tokio primitives.
- Cleanup and terminal outcomes are explicit, and every task remains recoverable through a full snapshot.

The boundaries and intended public task model come from `PHASE2-PLAN.md` sections 1, 3, and 5. The separation of aggregate tasks from leaf work is informed by `SCHEDULER-RESEARCH.md` sections 1-5.

## MVP behavior

- Copy, move, permanent-delete, and trash commands use bounded admission and
  stable FIFO dispatch. Priority values are retained for later policy but do
  not preempt work in v1.
- `SchedulerConfig` bounds aggregate concurrency, pending tasks, per-task
  selections, internal updates, frontend observations, and terminal history.
- `TaskHandle` provides a stable ID, atomic lifecycle, cooperative
  cancellation, a lossless-latest watch snapshot, and terminal waiting.
- Frontend observations may coalesce under pressure, but loss sets a persistent
  dirty signal. `next_event` then returns `ResyncRequired` and drains stale
  queued observations so actions/state can fetch the authoritative snapshot set.
- Copy uses a task-owned temporary sibling, chunked asynchronous I/O,
  throttled progress, attribute application, and rename publication. Failure
  or cancellation removes the unpublished temporary.
- Directory-tree copy and cross-filesystem directory move are intentionally
  deferred. Same-filesystem directory rename/delete/trash remain available.
