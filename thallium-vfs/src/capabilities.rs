//! Backend feature declarations.
//!
//! Adapted from the capability-oriented VFS surface described in
//! `FS-VFS-RESEARCH.md` section 2 and `PHASE2-PLAN.md` section 2.

/// Operations and guarantees advertised by a filesystem backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Capabilities {
	/// Whether a rename can atomically publish a staged file.
	pub atomic_rename:    bool,
	/// Whether the backend supports moving entries to a desktop trash.
	pub trash:            bool,
	/// Whether symbolic links are supported.
	pub symlink:          bool,
	/// Whether hard links are supported.
	pub hard_link:        bool,
	/// Whether sparse files can be preserved or created.
	pub sparse_files:     bool,
	/// Whether copy progress can be measured while copying.
	pub progressive_copy: bool,
}
