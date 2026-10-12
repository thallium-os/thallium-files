//! Renderer-neutral commands accepted by the single action/state owner.

use thallium_fs_core::{ConflictPolicy, EntryId, Location, NativePath, StateRevision};
use thallium_protocol::TaskId;

/// A command paired with the browser revision against which it was created.
///
/// Frontends that do not retain revisions can use
/// [`crate::Coordinator::dispatch_command`], which targets the latest state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandEnvelope {
	pub expected_revision: StateRevision,
	pub command:           Command,
}

/// Closed MVP command vocabulary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
	Navigate(Location),
	NavigateBack,
	NavigateForward,
	Refresh,
	SetSelection {
		selected: Vec<EntryId>,
		focused:  Option<EntryId>,
	},
	ToggleSelection(EntryId),
	ClearSelection,
	Copy {
		sources:         Vec<Location>,
		destination:     Location,
		conflict_policy: ConflictPolicy,
	},
	Move {
		sources:         Vec<Location>,
		destination:     Location,
		conflict_policy: ConflictPolicy,
	},
	Delete {
		targets: Vec<Location>,
	},
	Trash {
		targets: Vec<Location>,
	},
	/// Give `source` a new final path component in the same folder.
	Rename {
		source: Location,
		name:   NativePath,
	},
	CreateFolder {
		parent: Location,
		name:   NativePath,
	},
	/// Create an empty file, or a copy of `template` when one is given.
	CreateFile {
		parent:   Location,
		name:     NativePath,
		template: Option<Location>,
	},
	/// Place a symbolic link to each source inside `destination`.
	Link {
		sources:     Vec<Location>,
		destination: Location,
	},
	CancelTask(TaskId),
	DismissTask(TaskId),
}
