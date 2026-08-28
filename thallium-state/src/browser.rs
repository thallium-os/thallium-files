//! Top-level renderer-neutral model for the single MVP browser pane.
//!
//! This deliberately separates browser truth from Yazi's renderer-coupled
//! `Core`/`Mgr` shape documented in `WATCHER-STATE-RESEARCH.md` sections
//! 2.3-2.4.

use std::collections::BTreeMap;

use thallium_fs_core::{Location, StateRevision};
use thallium_protocol::{BrowserStateSnapshot, EntryIdDto, LocationDto, TaskId, TaskInfo};

use crate::{DirectoryState, NavigationHistory, SelectionState};

/// Complete browser state with one external owner.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BrowserState {
	/// Monotonic version advanced once per accepted semantic transition.
	pub revision:         StateRevision,
	/// Current backend-qualified directory, if initialized.
	pub current_location: Option<Location>,
	/// Current generation-tagged directory truth.
	pub directory:        DirectoryState,
	/// Current-directory selection and focus.
	pub selection:        SelectionState,
	/// Backward and forward navigation stacks.
	pub history:          NavigationHistory,
	/// Latest scheduler snapshots keyed by task identity.
	pub tasks:            BTreeMap<TaskId, TaskInfo>,
	/// Task insertion order, independent of map ordering.
	pub task_order:       Vec<TaskId>,
}

impl BrowserState {
	/// Returns tasks in stable insertion order.
	pub fn ordered_tasks(&self) -> impl Iterator<Item = &TaskInfo> {
		self.task_order.iter().filter_map(|id| self.tasks.get(id))
	}

	/// Builds an owned, bridge-safe snapshot for the frontend adapter.
	#[must_use]
	pub fn snapshot(&self) -> BrowserStateSnapshot {
		BrowserStateSnapshot {
			revision:         self.revision,
			current_location: self.current_location.as_ref().map(LocationDto::from),
			directory:        self.directory.snapshot(),
			selection:        self.selection.selected.iter().copied().map(EntryIdDto::from).collect(),
			tasks:            self.ordered_tasks().cloned().collect(),
			can_go_back:      self.history.can_go_back(),
			can_go_forward:   self.history.can_go_forward(),
		}
	}
}
