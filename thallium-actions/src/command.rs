//! Renderer-neutral commands accepted by the single action/state owner.

use thallium_fs_core::{ConflictPolicy, EntryId, Location, StateRevision};
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
	CancelTask(TaskId),
	DismissTask(TaskId),
}
