//! Owned scheduler observations and reducer events.

use thallium_fs_core::{FsError, Location};
use thallium_protocol::{TaskId, TaskInfo};

/// Result of cleaning a scheduler-owned unpublished path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CleanupOutcome {
	Succeeded,
	Failed(FsError),
}

/// Fine-grained facts accepted by [`crate::Ongoing::reduce`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerEvent {
	WorkDiscovered { task_id: TaskId, location: Location, bytes: u64 },
	ItemStarted { task_id: TaskId, location: Location },
	BytesAdvanced { task_id: TaskId, bytes: u64 },
	ItemSucceeded { task_id: TaskId, location: Location },
	ItemFailed { task_id: TaskId, location: Location, error: FsError },
	ItemSkipped { task_id: TaskId, location: Location },
	DiscoveryFinished { task_id: TaskId },
	DiscoveryFailed { task_id: TaskId, error: FsError },
	CleanupFinished { task_id: TaskId, outcome: CleanupOutcome },
}

impl WorkerEvent {
	pub(crate) const fn task_id(&self) -> TaskId {
		match self {
			Self::WorkDiscovered { task_id, .. }
			| Self::ItemStarted { task_id, .. }
			| Self::BytesAdvanced { task_id, .. }
			| Self::ItemSucceeded { task_id, .. }
			| Self::ItemFailed { task_id, .. }
			| Self::ItemSkipped { task_id, .. }
			| Self::DiscoveryFinished { task_id }
			| Self::DiscoveryFailed { task_id, .. }
			| Self::CleanupFinished { task_id, .. } => *task_id,
		}
	}
}

/// Coalescible task-registry observation consumed by actions/state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerEvent {
	Snapshot(Box<TaskInfo>),
	Dismissed(TaskId),
	/// One or more coalescible events could not enter the bounded queue. The
	/// consumer must call `Scheduler::snapshots` before applying later events.
	ResyncRequired,
}
