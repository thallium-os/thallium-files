//! Generation-tagged current-directory state consumed by pure reducers.
//!
//! The authoritative snapshot/incremental delta distinction comes from
//! `WATCHER-STATE-RESEARCH.md` sections 2.3 and 3 and `PHASE2-PLAN.md` section
//! 4. Entries are keyed for identity lookups and separately ordered so model
//! row order never depends on a map implementation.

use std::{cmp::Ordering, collections::{BTreeMap, BTreeSet}};

use thallium_fs_core::{EntryId, FileEntry, FsError, ListingGeneration, Location};
use thallium_protocol::{DirectoryEntryRow, DirectorySnapshot, FsErrorDto, LocationDto};

/// Current known truth for the single directory listing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirectoryState {
	/// Directory represented by this snapshot, if initialized.
	pub location:   Option<Location>,
	/// Listing generation used to reject stale results.
	pub generation: ListingGeneration,
	/// Entries keyed by stable identity.
	pub entries:    BTreeMap<EntryId, FileEntry>,
	/// Deterministic presentation order, containing each known ID once.
	pub order:      Vec<EntryId>,
	/// Whether the generation is an authoritative completed listing.
	pub complete:   bool,
	/// Latest listing failure, if the snapshot could not be completed.
	pub error:      Option<FsError>,
}

impl DirectoryState {
	/// Starts an empty listing for a newly requested location and generation.
	#[must_use]
	pub fn loading(location: Location, generation: ListingGeneration) -> Self {
		Self { location: Some(location), generation, ..Self::default() }
	}

	/// Replaces the entries and listing outcome while preserving the generation.
	pub fn replace(&mut self, entries: Vec<FileEntry>, complete: bool, error: Option<FsError>) {
		self.entries = entries.into_iter().map(|entry| (entry.id, entry)).collect();
		self.complete = complete;
		self.error = error;
		self.reorder();
	}

	/// Inserts or replaces one entry and restores deterministic ordering.
	pub fn upsert(&mut self, entry: FileEntry) {
		self.entries.insert(entry.id, entry);
		self.reorder();
	}

	/// Removes one entry by stable identity.
	pub fn remove(&mut self, id: EntryId) -> bool {
		let removed = self.entries.remove(&id).is_some();
		if removed {
			self.order.retain(|candidate| *candidate != id);
		}
		removed
	}

	/// Returns every currently known entry identity.
	#[must_use]
	pub fn ids(&self) -> BTreeSet<EntryId> { self.entries.keys().copied().collect() }

	/// Returns entries in deterministic model order.
	pub fn ordered_entries(&self) -> impl Iterator<Item = &FileEntry> {
		self.order.iter().filter_map(|id| self.entries.get(id))
	}

	/// Converts the current listing to its boundary-safe protocol snapshot.
	#[must_use]
	pub fn snapshot(&self) -> DirectorySnapshot {
		DirectorySnapshot {
			location:   self.location.as_ref().map_or_else(LocationDto::default, LocationDto::from),
			generation: self.generation.0,
			entries:    self.ordered_entries().map(DirectoryEntryRow::from).collect(),
			complete:   self.complete,
			error:      self.error.as_ref().map(FsErrorDto::from),
		}
	}

	fn reorder(&mut self) {
		let entries = &self.entries;
		self.order = entries.keys().copied().collect();
		self.order.sort_by(|left, right| compare_entries(&entries[left], &entries[right]));
	}
}

fn compare_entries(left: &FileEntry, right: &FileEntry) -> Ordering {
	left
		.name
		.as_bytes()
		.cmp(right.name.as_bytes())
		.then_with(|| left.location.path.as_bytes().cmp(right.location.path.as_bytes()))
		.then_with(|| left.id.cmp(&right.id))
}
