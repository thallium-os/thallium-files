//! Explicit metadata preservation intent for write operations.
//!
//! Adapted from the transferable attributes documented in
//! `FS-VFS-RESEARCH.md` section 1 and the staged-copy contract in
//! `PHASE2-PLAN.md` section 2.

use crate::{EntryMetadata, Timestamp};

/// Attributes that a filesystem operation may request independently.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FileAttrs {
	/// Requested Linux permission bits.
	pub permissions: Option<u32>,
	/// Requested owner user identifier.
	pub uid:         Option<u32>,
	/// Requested owner group identifier.
	pub gid:         Option<u32>,
	/// Requested access time.
	pub accessed:    Option<Timestamp>,
	/// Requested modification time.
	pub modified:    Option<Timestamp>,
}

impl FileAttrs {
	/// Captures the attributes ordinarily preserved by a local copy.
	#[must_use]
	pub fn from_metadata(metadata: &EntryMetadata) -> Self {
		Self {
			permissions: Some(metadata.permissions),
			uid:         Some(metadata.uid),
			gid:         Some(metadata.gid),
			accessed:    metadata.accessed,
			modified:    metadata.modified,
		}
	}

	/// Returns whether no attribute was requested.
	#[must_use]
	pub const fn is_empty(&self) -> bool {
		self.permissions.is_none()
			&& self.uid.is_none()
			&& self.gid.is_none()
			&& self.accessed.is_none()
			&& self.modified.is_none()
	}
}
