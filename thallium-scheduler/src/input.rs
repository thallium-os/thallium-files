//! Owned inputs for aggregate MVP filesystem tasks.

use thallium_fs_core::{ConflictPolicy, FileAttrs, Location};

use crate::Priority;

/// Options shared by copy and move tasks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskOptions {
	/// Accepted for forward compatibility; v1 dispatch is deliberately FIFO.
	pub priority:        Priority,
	pub conflict_policy: ConflictPolicy,
	pub attributes:      Option<FileAttrs>,
}

impl Default for TaskOptions {
	fn default() -> Self {
		Self {
			priority:        Priority::Normal,
			conflict_policy: ConflictPolicy::Ask,
			attributes:      None,
		}
	}
}

/// Copy selections into a destination directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyTask {
	pub sources:     Vec<Location>,
	pub destination: Location,
	pub options:     TaskOptions,
}

/// Move selections into a destination directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoveTask {
	pub sources:     Vec<Location>,
	pub destination: Location,
	pub options:     TaskOptions,
}

/// Permanently remove selections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteTask {
	pub targets:  Vec<Location>,
	pub priority: Priority,
}

/// Move selections to the desktop trash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrashTask {
	pub targets:  Vec<Location>,
	pub priority: Priority,
}

/// Public admission payload for the MVP scheduler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskInput {
	Copy(CopyTask),
	Move(MoveTask),
	Delete(DeleteTask),
	Trash(TrashTask),
}

impl TaskInput {
	pub(crate) fn validate(&self) -> Result<(), String> {
		let (locations, destination) = match self {
			Self::Copy(input) => (&input.sources, Some(&input.destination)),
			Self::Move(input) => (&input.sources, Some(&input.destination)),
			Self::Delete(input) => (&input.targets, None),
			Self::Trash(input) => (&input.targets, None),
		};
		if locations.is_empty() {
			return Err("a task must contain at least one source or target".into());
		}
		if locations.iter().any(|location| !location.is_absolute())
			|| destination.is_some_and(|location| !location.is_absolute())
		{
			return Err("scheduler locations must be absolute".into());
		}
		Ok(())
	}
}
