# thallium-state

Renderer-neutral browser state and pure reducers for the MVP's single pane,
current-directory snapshot, selection, navigation history, and task snapshots.

Boundary invariants:

- this crate contains no Qt/QML, terminal, renderer, viewport-geometry, Tokio
  runtime, channel, `JoinHandle`, callback, closure, or global singleton type;
- state is plain owned data with one external owner; only `thallium-actions`
  will coordinate mutation;
- reducers perform no I/O, rendering, prompting, process launch, or async work;
- reducers consume typed owned actions/deltas and return owned normalized
  changes plus explicit data-only effects;
- mutable state borrows never cross an `.await`; and
- filesystem entries are addressed by stable IDs/locations, not visible rows.

These boundaries follow `PHASE2-PLAN.md` sections 1 and 4. The separation from
Yazi's coupled `Core`, `Mgr`, and actor context is informed by
`WATCHER-STATE-RESEARCH.md` sections 2-3.
