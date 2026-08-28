//! Selection and focus represented by stable entry identities.
//!
//! The ID-based shape is required by `PHASE2-PLAN.md` section 4 and avoids the
//! row/viewport coupling identified in `WATCHER-STATE-RESEARCH.md` section 2.4.

use std::collections::BTreeSet;

use thallium_fs_core::EntryId;

/// Selection local to the current directory.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectionState {
	/// Selected entries, independent of sorting or visible row.
	pub selected: BTreeSet<EntryId>,
	/// Focused entry, if one is currently known.
	pub focused:  Option<EntryId>,
}

impl SelectionState {
	/// Removes IDs not present in the current directory.
	pub fn retain(&mut self, present: &BTreeSet<EntryId>) {
		self.selected.retain(|id| present.contains(id));
		if self.focused.is_some_and(|id| !present.contains(&id)) {
			self.focused = None;
		}
	}

	/// Moves selection and focus from a renamed entry ID to its replacement.
	pub fn rename(&mut self, old: EntryId, new: EntryId) {
		if self.selected.remove(&old) {
			self.selected.insert(new);
		}
		if self.focused == Some(old) {
			self.focused = Some(new);
		}
	}
}
