# thallium-fs-core

Owned filesystem value types and policies for the Thallium 81 backend: native
paths, locations, entry metadata, attributes, stable IDs, deltas, error codes,
conflict policy, and metadata signatures.

## Boundary

This crate contains owned data only. It has no async runtime, Qt/QML or other
renderer types, Tokio channels, callbacks, mutable global singletons, or
process-global current-directory behavior. Higher layers receive explicit
`Location` values for all filesystem work.

The boundary is defined by `PHASE2-PLAN.md`; type shapes are informed by
`FS-VFS-RESEARCH.md` section 1 and are original Thallium scaffolding rather than
Yazi source or dependencies.
