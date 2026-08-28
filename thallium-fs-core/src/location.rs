//! Backend-qualified filesystem locations.
//!
//! This is the small future-backend seam selected in `PHASE2-PLAN.md` section
//! 2, adapted from the more extensive URL/authority model documented in
//! `FS-VFS-RESEARCH.md` section 1.

use crate::NativePath;

/// Stable identifier for a registered filesystem backend.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackendId(pub u32);

impl BackendId {
	/// The only backend registered in v1.
	pub const LOCAL: Self = Self(0);
}

impl Default for BackendId {
	fn default() -> Self { Self::LOCAL }
}

/// A logical filesystem location owned independently of any backend handle.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Location {
	/// Backend responsible for this location.
	pub backend: BackendId,
	/// Exact backend-native path bytes.
	pub path:    NativePath,
}

impl Location {
	/// Creates a backend-qualified location.
	#[must_use]
	pub const fn new(backend: BackendId, path: NativePath) -> Self { Self { backend, path } }

	/// Creates a location in the v1 local backend.
	#[must_use]
	pub const fn local(path: NativePath) -> Self { Self::new(BackendId::LOCAL, path) }

	/// Returns a lexically normalized location without accessing the backend.
	#[must_use]
	pub fn normalize(&self) -> Self { Self::new(self.backend, self.path.normalize()) }

	/// Joins a native child path while retaining the backend identity.
	#[must_use]
	pub fn join(&self, child: impl AsRef<NativePath>) -> Self {
		Self::new(self.backend, self.path.join(child))
	}

	/// Returns the lexical parent location.
	#[must_use]
	pub fn parent(&self) -> Option<Self> {
		self.path.parent().map(|path| Self::new(self.backend, path))
	}

	/// Returns whether this is an absolute backend-native path.
	#[must_use]
	pub fn is_absolute(&self) -> bool { self.path.is_absolute() }
}

impl From<NativePath> for Location {
	fn from(path: NativePath) -> Self { Self::local(path) }
}
