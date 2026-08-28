//! Metadata-derived cache invalidation signatures.
//!
//! Informed by `FileSig`/`ChaSig` in `FS-VFS-RESEARCH.md` section 1. This type
//! is explicitly a cache heuristic, not a content-integrity digest.

use crate::{EntryMetadata, FileEntry, FileKind, Location, Timestamp};

/// Cheap metadata signature used to notice potentially stale cached data.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MetadataSignature {
	/// Location whose metadata was sampled.
	pub location:    Location,
	/// Content length at sampling time.
	pub size:        u64,
	/// Observed entry kind.
	pub kind:        FileKind,
	/// Linux permission and type bits.
	pub permissions: u32,
	/// Filesystem device identifier.
	pub device:      u64,
	/// Filesystem inode identifier.
	pub inode:       u64,
	/// Last content-modification time, when available.
	pub modified:    Option<Timestamp>,
	/// Last metadata-change time, when available.
	pub changed:     Option<Timestamp>,
	/// Creation/birth time, when available.
	pub created:     Option<Timestamp>,
}

impl MetadataSignature {
	/// Captures the cheap change-detection fields for an entry.
	#[must_use]
	pub fn from_entry(entry: &FileEntry) -> Self {
		Self::new(entry.location.clone(), &entry.metadata)
	}

	/// Captures a signature for metadata observed at an explicit location.
	#[must_use]
	pub fn new(location: Location, metadata: &EntryMetadata) -> Self {
		Self {
			location,
			size: metadata.size,
			kind: metadata.kind,
			permissions: metadata.permissions,
			device: metadata.device,
			inode: metadata.inode,
			modified: metadata.modified,
			changed: metadata.changed,
			created: metadata.created,
		}
	}

	/// Returns whether the current signature represents a different observation.
	#[must_use]
	pub fn differs_from(&self, previous: &Self) -> bool { self != previous }
}

#[cfg(test)]
mod tests {
	use super::MetadataSignature;
	use crate::{BackendId, EntryMetadata, FileKind, Location, NativePath, Timestamp};

	fn metadata() -> EntryMetadata {
		EntryMetadata {
			kind:        FileKind::File,
			size:        4,
			permissions: 0o100644,
			uid:         1,
			gid:         2,
			accessed:    None,
			created:     None,
			changed:     Some(Timestamp::new(1, 0)),
			modified:    Some(Timestamp::new(1, 0)),
			device:      3,
			inode:       4,
			link_count:  1,
		}
	}

	#[test]
	fn detects_relevant_metadata_changes() {
		let location = Location::new(BackendId::LOCAL, NativePath::from(&b"/tmp/a"[..]));
		let original = MetadataSignature::new(location.clone(), &metadata());
		let mut changed = metadata();
		changed.size += 1;
		assert!(MetadataSignature::new(location.clone(), &changed).differs_from(&original));
		assert!(!MetadataSignature::new(location, &metadata()).differs_from(&original));
	}
}
