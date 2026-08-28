//! One owned feedback vocabulary for VFS, scheduler, and watcher results.

use std::fmt;

use thallium_fs_core::{FileEntry, FsDelta, FsErrorCode, ListingGeneration, Location};
use thallium_protocol::{TaskId, TaskInfo};
use thallium_scheduler::SchedulerEvent;
use thallium_watcher::WatchSetDiff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectKind {
	Vfs,
	Scheduler,
	Watcher,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectError {
	pub kind:     EffectKind,
	pub code:     Option<FsErrorCode>,
	pub message:  String,
	pub location: Option<Location>,
}

impl fmt::Display for EffectError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(formatter, "{:?} effect failed: {}", self.kind, self.message)
	}
}

impl std::error::Error for EffectError {}

/// Every asynchronous service observation enters the state owner in this form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectFeedback {
	DirectoryScanned {
		location:   Location,
		generation: ListingGeneration,
		entries:    Vec<FileEntry>,
	},
	DirectoryScanFailed {
		location:   Location,
		generation: ListingGeneration,
		error:      thallium_fs_core::FsError,
	},
	WatcherReconciled {
		deltas: Vec<FsDelta>,
	},
	WatchSetSynchronized(WatchSetDiff),
	TaskSubmitted(TaskInfo),
	SchedulerEvent(SchedulerEvent),
	/// Authoritative replacement obtained immediately after
	/// `SchedulerEvent::ResyncRequired`.
	SchedulerResynchronized(Vec<TaskInfo>),
	TaskCancelled(TaskId),
	TaskDismissed(TaskId),
	ServiceFailed(EffectError),
}

pub type EffectResult = Result<EffectFeedback, EffectError>;
