//! Renderer-neutral filesystem observation for the Thallium 81 backend.
//!
//! The service boundary follows `PHASE2-PLAN.md` sections 1 and 4. Its watched
//! set, batching, restat, and refresh state-machine shapes are informed by
//! `WATCHER-STATE-RESEARCH.md` section 1.

pub mod batch;
pub mod notification;
pub mod refresher;
pub mod reporter;
pub mod watched_set;
pub mod watchee;
pub mod watcher;

pub use batch::NotificationBatcher;
pub use notification::{NotificationBatch, NotificationHint, WatcherConfig};
pub use refresher::{RefreshEntry, RefreshRequest, RefreshResult, RefreshState, Refresher};
pub use reporter::{ReportError, Reporter};
pub use watched_set::{WatchSetDiff, WatchedSet};
pub use watchee::Watchee;
pub use watcher::{DirectorySnapshot, ReconcileResult, Watcher};
