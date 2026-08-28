//! Staged-copy request and result values.
//!
//! The final staged-publication contract is described in `PHASE2-PLAN.md`
//! section 2; Yazi's contrasting copy behavior is documented in
//! `FS-VFS-RESEARCH.md` sections 1 and 2.

use thallium_fs_core::{ConflictPolicy, FileAttrs, Location};

/// Policy knobs for copying one file through a task-owned staging path.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StagedCopyOptions {
	/// Resolution policy if the destination already exists.
	pub conflict_policy: ConflictPolicy,
	/// Attributes to apply before publishing the staged file.
	pub attributes:      Option<FileAttrs>,
	/// Whether the implementation should preserve sparse-file layout.
	pub preserve_sparse: bool,
}

/// Facts returned after a staged file has been published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedCopyOutcome {
	/// Final published destination.
	pub destination:  Location,
	/// Number of content bytes copied.
	pub bytes_copied: u64,
	/// Whether a new destination was published (`false` for `Skip`).
	pub published:    bool,
}

/// Policy knobs for a rename with a copy-and-delete cross-device fallback.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MoveOptions {
	/// Copy behavior used only when a direct rename reports `EXDEV`.
	pub copy:   StagedCopyOptions,
	/// Whether a cross-device directory move may recursively delete its source.
	///
	/// The MVP staged-copy implementation supports files, so this is normally
	/// `false`; the field keeps deletion intent explicit at the boundary.
	pub delete: crate::DeleteOptions,
}

/// Result of moving an entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoveOutcome {
	/// Actual final destination, including conflict-policy renames.
	pub destination:  Location,
	/// Bytes copied by an `EXDEV` fallback, or zero for a direct rename.
	pub bytes_copied: u64,
	/// Whether the move required copy-and-delete rather than a rename.
	pub copied:       bool,
	/// Whether a destination was published (`false` for `Skip`).
	pub published:    bool,
}
