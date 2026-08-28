//! Minimal browser snapshots and revisioned model changes.
//!
//! The MVP has one current directory rather than tabs. These owned values are
//! suitable for mapping into Qt list models, but contain no Qt objects or
//! state-owner references.

use thallium_fs_core::StateRevision;

use crate::{DirectoryEntryRow, DirectorySnapshot, EntryIdDto, FsErrorDto, LocationDto, TaskId, TaskInfo};

/// Complete recovery snapshot consumed by the frontend adapter.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BrowserStateSnapshot {
	pub revision:         StateRevision,
	pub current_location: Option<LocationDto>,
	pub directory:        DirectorySnapshot,
	pub selection:        Vec<EntryIdDto>,
	pub tasks:            Vec<TaskInfo>,
	pub can_go_back:      bool,
	pub can_go_forward:   bool,
}

/// One normalized frontend-model change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateChange {
	LocationChanged { location: LocationDto, can_go_back: bool, can_go_forward: bool },
	DirectoryReplaced(DirectorySnapshot),
	EntriesUpserted(Vec<DirectoryEntryRow>),
	EntriesRemoved(Vec<EntryIdDto>),
	SelectionChanged(Vec<EntryIdDto>),
	DirectoryErrorChanged(Option<FsErrorDto>),
	TaskUpserted(TaskInfo),
	TaskRemoved(TaskId),
}

/// Ordered changes advancing a consumer between two exact state revisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateDelta {
	pub from_revision: StateRevision,
	pub revision:      StateRevision,
	pub changes:       Vec<StateChange>,
}

impl StateDelta {
	/// Returns whether this delta can be applied without a missed update.
	#[must_use]
	pub fn is_applicable_to(&self, revision: StateRevision) -> bool {
		self.from_revision == revision && self.revision > revision
	}
}

/// Name retained for the backend's full-snapshot API.
pub type StateSnapshot = BrowserStateSnapshot;

#[cfg(test)]
mod tests {
	use thallium_fs_core::StateRevision;

	use super::StateDelta;

	#[test]
	fn delta_requires_an_exact_contiguous_revision() {
		let delta = StateDelta {
			from_revision: StateRevision(4),
			revision:      StateRevision(5),
			changes:       Vec::new(),
		};

		assert!(delta.is_applicable_to(StateRevision(4)));
		assert!(!delta.is_applicable_to(StateRevision(3)));
		assert!(!delta.is_applicable_to(StateRevision(5)));
	}

	#[test]
	fn delta_must_advance_the_revision() {
		let delta = StateDelta {
			from_revision: StateRevision(4),
			revision:      StateRevision(4),
			changes:       Vec::new(),
		};

		assert!(!delta.is_applicable_to(StateRevision(4)));
	}
}
