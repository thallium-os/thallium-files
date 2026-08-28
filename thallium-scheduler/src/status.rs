//! Internal aggregate task lifecycle.

use thallium_protocol::ConflictId;

/// Terminal outcome retained with a completed task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskOutcome {
	Succeeded,
	CompletedWithErrors,
	Failed,
	Cancelled,
}

/// Internal lifecycle of one aggregate user task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskStatus {
	Queued,
	Discovering,
	Running,
	WaitingForDecision { conflict_id: ConflictId },
	Cancelling,
	Finished(TaskOutcome),
}

impl TaskStatus {
	/// Returns whether the task can no longer change lifecycle state.
	#[must_use]
	pub const fn is_terminal(self) -> bool { matches!(self, Self::Finished(_)) }

	pub(crate) const fn atomic_value(self) -> u8 {
		match self {
			Self::Queued => 0,
			Self::Discovering | Self::Running => 1,
			Self::WaitingForDecision { .. } => 2,
			Self::Cancelling => 3,
			Self::Finished(TaskOutcome::Succeeded) => 4,
			Self::Finished(TaskOutcome::CompletedWithErrors | TaskOutcome::Failed) => 5,
			Self::Finished(TaskOutcome::Cancelled) => 6,
		}
	}
}
