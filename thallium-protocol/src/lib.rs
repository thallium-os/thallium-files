//! Owned values exposed by the Thallium backend to its cxx-qt adapter.
//!
//! This is intentionally narrower than a serialized transport protocol. It
//! contains only current-directory rows, browser snapshots/deltas, and task
//! progress values needed by the MVP QML models.

pub mod ids;
pub mod location;
pub mod state;
pub mod task;

pub use ids::{ConflictId, EntryIdDto, TabId, TaskId, TaskRevision};
pub use location::{DirectoryEntryRow, DirectorySnapshot, EntryDto, EntryKind, FsErrorDto, LocationDto, PathBytes, TimestampDto};
pub use state::{BrowserStateSnapshot, StateChange, StateDelta, StateSnapshot};
pub use task::{TaskInfo, TaskIssue, TaskKind, TaskProgress, TaskSnapshot, TaskState, TaskStatus};
pub use thallium_fs_core::{EntryId, ListingGeneration, StateRevision};
