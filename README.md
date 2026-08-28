# thallium-files

> File manager for Thallium 81. Optional package, not an ISO default.

## What it does

Qt/QML file manager on a Rust backend. Eight crates: a VFS abstraction over
the local filesystem, a change-batching watcher, a task scheduler for
copy/move/delete/trash, reducer-based browser state, and an action
coordinator the QML frontend drives through a `cxx-qt` bridge.

## Quick Start

```sh
cargo run -p thallium-backend --features qt
```

Headless build of the backend crates only:

```sh
cargo check --workspace
cargo test --workspace
```

## Status

The backend is exercised end to end by `thallium-backend/tests/end_to_end.rs`,
which drives a real `Backend` over a temporary directory: copy, move, delete,
conflict policies, navigation history, selection, refresh, task dismissal and
shutdown all round-trip through coordinator, scheduler and VFS.

Known gaps, each pinned by a test rather than left silent:

- Copy handles regular files only. Delete recurses; copy does not.
- `LocalFs` is the only VFS backend.
- Trashing depends on an XDG trash directory being present and reachable.
- The watcher's reporter is a stub, so refresh is explicit rather than live.

## Installation

Not packaged yet. No `debian/` at repo root.

## Architecture

```
thallium-fs-core    types, paths, metadata, deltas, errors
thallium-protocol   frontend-visible DTOs (depends on fs-core)
thallium-vfs        VfsFacade trait + LocalFs backend
thallium-watcher    notify-backed watcher, batching, refresh
thallium-scheduler  task queue, progress, cancellation
thallium-state      browser state reducer + history
thallium-actions    command validation, dispatch, feedback pump
thallium-backend    cxx-qt bridge, QML module, binary
```

The `qt` feature is off by default so the backend crates build and test
without a Qt toolchain.

## Requirements

Rust 1.95+. Qt 6 development packages for the `qt` feature.

## License

GPL-3.0-or-later.
