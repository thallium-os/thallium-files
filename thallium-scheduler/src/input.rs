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

/// Give one entry a new location in the same folder; never replaces an
/// existing entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameTask {
	pub source:      Location,
	pub destination: Location,
}

/// Create one empty folder, or one file that is empty or a copy of
/// `template`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateTask {
	pub target:   Location,
	pub template: Option<Location>,
}

/// Place a symbolic link to each source inside `destination`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkTask {
	pub sources:     Vec<Location>,
	pub destination: Location,
}

/// Public admission payload for the MVP scheduler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskInput {
	Copy(CopyTask),
	Move(MoveTask),
	Delete(DeleteTask),
	Trash(TrashTask),
	Rename(RenameTask),
	CreateFolder(CreateTask),
	CreateFile(CreateTask),
	Link(LinkTask),
}

impl TaskInput {
	pub(crate) fn validate(&self) -> Result<(), String> {
		let (locations, destination) = match self {
			Self::Copy(input) => (input.sources.clone(), Some(&input.destination)),
			Self::Move(input) => (input.sources.clone(), Some(&input.destination)),
			Self::Delete(input) => (input.targets.clone(), None),
			Self::Trash(input) => (input.targets.clone(), None),
			Self::Rename(input) => (vec![input.source.clone()], Some(&input.destination)),
			Self::CreateFolder(input) | Self::CreateFile(input) => {
				(vec![input.target.clone()], input.template.as_ref())
			}
			Self::Link(input) => (input.sources.clone(), Some(&input.destination)),
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
