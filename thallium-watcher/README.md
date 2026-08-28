# thallium-watcher

Renderer-neutral filesystem observation for the Thallium 81 backend. The crate
owns desired watch sets, bounded notification batching, VFS-backed restat and
reconciliation, and coalesced directory refresh state.

## Boundary invariants

- Emits typed, owned filesystem deltas; it never mutates browser state or calls
  Qt, QML, a renderer, or UI actions.
- Uses only explicit backend-qualified locations and never changes process-global
  current working directory.
- Treats OS notifications as invalidation hints and derives authoritative state
  through `thallium-vfs`.
- Keeps runtime channels internal and bounded; no channel, callback, or backend
  handle appears in filesystem-core values or frontend DTOs.
- Owns no rendering geometry, mutable application state, or frontend decision.

These boundaries are selected in `PHASE2-PLAN.md` sections 1 and 4. The module
shapes are informed by `WATCHER-STATE-RESEARCH.md` section 1 without copying
Yazi source or depending on a Yazi crate.

## MVP platform scope

The local adapter uses `notify::RecommendedWatcher`, which is inotify-backed on
the supported Linux target. Watches are non-recursive and intended for the one
currently displayed directory. There is no polling fallback in the MVP;
`Watchee::Virtual` is retained only as a future backend seam.
