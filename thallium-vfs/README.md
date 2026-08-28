# thallium-vfs

Renderer- and Qt-neutral asynchronous filesystem façade, initially backed by
the local Linux filesystem.

## Boundary invariants

- Every operation receives an explicit `Location`; this crate never changes or
  relies on process-global current-directory state.
- The VFS does not call Qt, render, prompt, or mutate browser state. It returns
  typed values and errors to its caller.
- Backend handles and asynchronous implementation details remain behind the VFS
  boundary and do not become protocol DTOs.
- The initial local backend is capability-oriented. Future remote backends must
  honor the same contracts without changing scheduler or state DTOs.
- The MVP local backend implements scanning, metadata, file handles, directory
  creation, links, staged file copies, rename-with-`EXDEV` fallback, trash, and
  permanent deletion. Cross-device directory moves and sparse preservation are
  intentionally deferred.
- Cancellation is coordinated by the scheduler. The VFS exposes file handles
  and task-owned staging paths so a cancelled chunked copy can remove its
  staging file without ever publishing partial content.
