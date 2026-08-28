# thallium-backend

Composition root and Qt adapter for the Thallium 81 file manager. It owns the
Tokio runtime, the single action/state loop, scheduler, watcher, and local VFS.

The default feature builds and tests the renderer-neutral backend. The `qt`
feature also builds `thallium-file-manager`, a Qt Quick Controls 2 application:

```sh
QMAKE=/path/to/Qt/6.x/gcc_64/bin/qmake \
  cargo run -p thallium-backend --features qt --bin thallium-file-manager
```

The MVP starts at `$HOME`, supports one local directory pane, back/forward/up
navigation, multi-selection, copy/move/trash/permanent-delete commands, and a
visible cancellable task list. Paths remain byte-preserving below the adapter;
the QML address bar is necessarily a Unicode presentation/input surface.

## Boundary invariants

- This crate only composes services; filesystem, scheduling, watching, and
  pure state behavior remain in their lower crates.
- Runtime handles, channels, VFS handles, mutable state references, and Qt
  objects never cross the frontend-neutral backend boundary.
- The Qt adapter consumes owned `thallium-protocol` snapshots.
- Only the action coordinator mutates browser state; the backend routes bounded
  requests to its single async owner task.
- No operation consults or changes process-global current working directory.
