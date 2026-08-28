//! Boundary-safe task rows for the QML task model.
//!
//! Raw counters are retained so the bridge can expose progress without tying
//! scheduler internals, channels, atomics, or cancellation handles to Qt.

use thallium_fs_core::FsErrorCode;

use crate::{LocationDto, TaskId, TaskRevision};

/// Filesystem operations exposed by the MVP.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskKind {
	Copy,
	Move,
	Delete,
	Trash,
}

/// Frontend-visible lifecycle of one aggregate task.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TaskStatus {
	Pending,
	Started,
	Cancelling,
	Succeeded,
	Failed,
	Cancelled,
}

impl TaskStatus {
	/// Returns whether no further progress is expected.
	#[must_use]
	pub const fn is_terminal(self) -> bool {
		matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
	}

	/// Returns whether this row represents active work.
	#[must_use]
	pub const fn is_active(self) -> bool {
		matches!(self, Self::Pending | Self::Started | Self::Cancelling)
	}
}

/// Raw aggregate progress facts published at a throttled rate by the scheduler.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TaskProgress {
	pub items_completed: u64,
	pub items_total:     Option<u64>,
	pub bytes_completed: u64,
	pub bytes_total:     Option<u64>,
}

impl TaskProgress {
	/// Returns progress in basis points (`0..=10_000`) without floating-point
	/// rounding or division-by-zero behavior at the bridge boundary.
	#[must_use]
	pub fn basis_points(self) -> u16 {
		let ratio = match self.bytes_total {
			Some(total) if total > 0 => Some((self.bytes_completed, total)),
			_ => self.items_total.filter(|total| *total > 0).map(|total| (self.items_completed, total)),
		};

		let Some((completed, total)) = ratio else { return 0 };
		let scaled = u128::from(completed.min(total)) * 10_000 / u128::from(total);
		scaled as u16
	}

	/// Returns whether every known unit has completed.
	#[must_use]
	pub fn is_complete(self) -> bool {
		let items = self.items_total.filter(|total| *total > 0);
		let bytes = self.bytes_total.filter(|total| *total > 0);
		let has_known_work = items.is_some() || bytes.is_some();
		has_known_work
			&& items.is_none_or(|total| self.items_completed >= total)
			&& bytes.is_none_or(|total| self.bytes_completed >= total)
	}
}

/// One user-presentable failure associated with a task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskIssue {
	pub code:     FsErrorCode,
	pub message:  String,
	pub location: Option<LocationDto>,
	pub os_code:  Option<i32>,
}

/// Complete owned row for the QML task list model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskInfo {
	pub id:            TaskId,
	pub revision:      TaskRevision,
	pub kind:          TaskKind,
	pub status:        TaskStatus,
	pub title:         String,
	pub progress:      TaskProgress,
	pub current_item:  Option<LocationDto>,
	pub issue:         Option<TaskIssue>,
	pub created_at_ms: u64,
	pub updated_at_ms: u64,
}

impl TaskInfo {
	/// Returns whether the task may currently accept a cancellation request.
	#[must_use]
	pub const fn can_cancel(&self) -> bool {
		matches!(self.status, TaskStatus::Pending | TaskStatus::Started)
	}
}

/// Name retained for state snapshots and scheduler observations.
pub type TaskSnapshot = TaskInfo;

/// Compatibility name used by early scheduler scaffolding.
pub type TaskState = TaskStatus;

#[cfg(test)]
mod tests {
	use super::{TaskProgress, TaskStatus};

	#[test]
	fn byte_progress_is_clamped_and_precise() {
		let halfway = TaskProgress {
			items_completed: 0,
			items_total:     None,
			bytes_completed: 5,
			bytes_total:     Some(10),
		};
		assert_eq!(halfway.basis_points(), 5_000);
		assert!(!halfway.is_complete());

		let beyond_total = TaskProgress { bytes_completed: 11, ..halfway };
		assert_eq!(beyond_total.basis_points(), 10_000);
		assert!(beyond_total.is_complete());
	}

	#[test]
	fn item_progress_is_used_when_byte_total_is_unknown() {
		let progress = TaskProgress {
			items_completed: 1,
			items_total:     Some(4),
			bytes_completed: 999,
			bytes_total:     None,
		};
		assert_eq!(progress.basis_points(), 2_500);
		assert!(!progress.is_complete());
	}

	#[test]
	fn unknown_and_empty_totals_do_not_claim_completion() {
		assert_eq!(TaskProgress::default().basis_points(), 0);
		assert!(!TaskProgress::default().is_complete());
		let empty = TaskProgress { bytes_total: Some(0), ..TaskProgress::default() };
		assert_eq!(empty.basis_points(), 0);
		assert!(!empty.is_complete());
	}

	#[test]
	fn all_known_progress_dimensions_must_complete() {
		let progress = TaskProgress {
			items_completed: 1,
			items_total:     Some(1),
			bytes_completed: 5,
			bytes_total:     Some(10),
		};

		assert!(!progress.is_complete());
	}

	#[test]
	fn task_status_lifecycle_has_explicit_terminal_states() {
		assert!(TaskStatus::Started.is_active());
		assert!(!TaskStatus::Started.is_terminal());
		assert!(TaskStatus::Cancelled.is_terminal());
		assert!(!TaskStatus::Cancelled.is_active());
	}
}
