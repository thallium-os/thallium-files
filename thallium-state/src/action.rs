//! Closed, owned inputs accepted by the pure browser-state reducer.
//!
//! The split-phase typed-action pattern is informed by
//! `WATCHER-STATE-RESEARCH.md` sections 2.2 and 3. These values contain no
//! callbacks, runtime handles, senders, or renderer objects.

use thallium_fs_core::{EntryId, FileEntry, FsDelta, FsError, ListingGeneration, Location};
use thallium_protocol::{TaskId, TaskSnapshot};

/// An owned semantic input to the browser state machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateAction {
	/// Navigate to an explicit directory and add the old location to history.
	NavigateTo(Location),
	/// Navigate to the most recent location in backward history.
	NavigateBack,
	/// Navigate to the most recent location in forward history.
	NavigateForward,
	/// Accept a generation-tagged directory scan result.
	DirectoryScanned {
		/// Directory scanned by the VFS.
		location:   Location,
		/// Generation requested by the state owner.
		generation: ListingGeneration,
		/// Complete or partial observed entries.
		entries:    Vec<FileEntry>,
		/// Whether this result is an authoritative completed listing.
		complete:   bool,
		/// Listing failure, if the scan could not complete.
		error:      Option<FsError>,
	},
	/// Apply one watcher-produced normalized filesystem delta.
	ApplyFsDelta(FsDelta),
	/// Replace selection and focus using stable entry identities.
	SetSelection {
		/// Selected entry identities.
		selected: Vec<EntryId>,
		/// Focused entry identity, if any.
		focused:  Option<EntryId>,
	},
	/// Toggle one entry's selected state and focus it.
	ToggleSelection(EntryId),
	/// Remove every selected and focused entry.
	ClearSelection,
	/// Insert or replace a scheduler-owned task snapshot.
	UpdateTask(TaskSnapshot),
	/// Remove a retained task snapshot.
	RemoveTask(TaskId),
}
