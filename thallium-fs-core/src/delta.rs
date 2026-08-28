//! Owned filesystem changes consumed by watcher reconciliation and state
//! reducers.
//!
//! The renderer-neutral shape follows `PHASE2-PLAN.md` sections 2 and 4 and is
//! informed by the change-batching behavior documented in
//! `FS-VFS-RESEARCH.md` section 1.

use std::collections::{BTreeMap, BTreeSet};

use crate::{EntryId, FileEntry, FsError, ListingGeneration, Location};

/// A normalized change within a directory snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FsChange {
	/// Insert a newly observed directory entry.
	Created(Box<FileEntry>),
	/// Replace an existing entry's metadata or presentation fields.
	Updated(Box<FileEntry>),
	/// Remove an entry that is no longer present.
	Deleted {
		/// Stable identity of the removed entry.
		id:       EntryId,
		/// Last known location of the removed entry.
		location: Location,
	},
	/// Move one observed entry to a new location.
	Renamed {
		/// Identity at the old location.
		old_id: EntryId,
		/// Last known old location.
		from:   Location,
		/// Complete observation at the new location.
		entry:  Box<FileEntry>,
	},
	/// Replace a directory listing with a complete observed snapshot.
	Reset(Vec<FileEntry>),
	/// Surface a reconciliation failure without fabricating state.
	Error(FsError),
}

/// A generation-tagged change for one parent directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsDelta {
	/// Directory whose snapshot is affected.
	pub parent:     Location,
	/// Listing generation used to reject stale results.
	pub generation: ListingGeneration,
	/// Normalized change to reduce.
	pub change:     FsChange,
}

impl FsDelta {
	/// Computes deterministic create/update/delete/rename changes between two
	/// complete snapshots of the same parent directory.
	///
	/// Exact entry IDs are compared first. Unmatched entries are recognized as a
	/// rename only when a device/inode identity occurs exactly once on each side;
	/// this avoids falsely pairing ambiguous hard links.
	#[must_use]
	pub fn between(
		parent: Location,
		generation: ListingGeneration,
		previous: &[FileEntry],
		current: &[FileEntry],
	) -> Vec<Self> {
		let old: BTreeMap<_, _> = previous.iter().map(|entry| (entry.id, entry)).collect();
		let new: BTreeMap<_, _> = current.iter().map(|entry| (entry.id, entry)).collect();
		let mut matched_old = BTreeSet::new();
		let mut matched_new = BTreeSet::new();
		let mut changes = Vec::new();

		for (id, entry) in &new {
			let Some(prior) = old.get(id) else { continue };
			matched_old.insert(*id);
			matched_new.insert(*id);
			if prior.location != entry.location {
				changes.push(FsChange::Renamed {
					old_id: *id,
					from:   prior.location.clone(),
					entry:  Box::new((*entry).clone()),
				});
			} else if *prior != *entry {
				changes.push(FsChange::Updated(Box::new((*entry).clone())));
			}
		}

		type ObjectKey = (crate::BackendId, u64, u64);
		let mut old_objects: BTreeMap<ObjectKey, Vec<&FileEntry>> = BTreeMap::new();
		let mut new_objects: BTreeMap<ObjectKey, Vec<&FileEntry>> = BTreeMap::new();
		for entry in previous.iter().filter(|entry| !matched_old.contains(&entry.id)) {
			old_objects.entry(object_key(entry)).or_default().push(entry);
		}
		for entry in current.iter().filter(|entry| !matched_new.contains(&entry.id)) {
			new_objects.entry(object_key(entry)).or_default().push(entry);
		}

		for (key, old_entries) in &old_objects {
			let Some(new_entries) = new_objects.get(key) else { continue };
			if let ([prior], [entry]) = (old_entries.as_slice(), new_entries.as_slice()) {
				matched_old.insert(prior.id);
				matched_new.insert(entry.id);
				changes.push(FsChange::Renamed {
					old_id: prior.id,
					from:   prior.location.clone(),
					entry:  Box::new((*entry).clone()),
				});
			}
		}

		for entry in previous.iter().filter(|entry| !matched_old.contains(&entry.id)) {
			changes.push(FsChange::Deleted { id: entry.id, location: entry.location.clone() });
		}
		for entry in current.iter().filter(|entry| !matched_new.contains(&entry.id)) {
			changes.push(FsChange::Created(Box::new(entry.clone())));
		}

		changes.into_iter().map(|change| Self { parent: parent.clone(), generation, change }).collect()
	}
}

fn object_key(entry: &FileEntry) -> (crate::BackendId, u64, u64) {
	(entry.location.backend, entry.metadata.device, entry.metadata.inode)
}

#[cfg(test)]
mod tests {
	use super::{FsChange, FsDelta};
	use crate::{BackendId, EntryMetadata, FileEntry, FileKind, ListingGeneration, Location, NativePath};

	fn entry(path: &[u8], inode: u64, size: u64) -> FileEntry {
		let location = Location::new(BackendId::LOCAL, NativePath::from(path));
		FileEntry::new(
			location,
			EntryMetadata {
				kind: FileKind::File,
				size,
				permissions: 0o100644,
				uid: 1,
				gid: 1,
				accessed: None,
				created: None,
				changed: None,
				modified: None,
				device: 2,
				inode,
				link_count: 1,
			},
			None,
		)
	}

	#[test]
	fn computes_create_update_delete_and_rename() {
		let parent = Location::local(NativePath::from(&b"/tmp"[..]));
		let old =
			vec![entry(b"/tmp/updated", 1, 1), entry(b"/tmp/deleted", 2, 1), entry(b"/tmp/old", 3, 1)];
		let current =
			vec![entry(b"/tmp/updated", 1, 2), entry(b"/tmp/created", 4, 1), entry(b"/tmp/new", 3, 1)];
		let deltas = FsDelta::between(parent, ListingGeneration(7), &old, &current);

		assert_eq!(deltas.len(), 4);
		assert!(deltas.iter().any(|delta| matches!(delta.change, FsChange::Updated(_))));
		assert!(deltas.iter().any(|delta| matches!(delta.change, FsChange::Created(_))));
		assert!(deltas.iter().any(|delta| matches!(delta.change, FsChange::Deleted { .. })));
		assert!(deltas.iter().any(|delta| matches!(delta.change, FsChange::Renamed { .. })));
	}

	#[test]
	fn ambiguous_hard_links_are_not_reported_as_renames() {
		let parent = Location::local(NativePath::from(&b"/tmp"[..]));
		let old = vec![entry(b"/tmp/a", 5, 1), entry(b"/tmp/b", 5, 1)];
		let current = vec![entry(b"/tmp/c", 5, 1), entry(b"/tmp/d", 5, 1)];
		let deltas = FsDelta::between(parent, ListingGeneration(1), &old, &current);
		assert_eq!(
			deltas.iter().filter(|delta| matches!(delta.change, FsChange::Deleted { .. })).count(),
			2
		);
		assert_eq!(
			deltas.iter().filter(|delta| matches!(delta.change, FsChange::Created(_))).count(),
			2
		);
		assert!(!deltas.iter().any(|delta| matches!(delta.change, FsChange::Renamed { .. })));
	}
}
