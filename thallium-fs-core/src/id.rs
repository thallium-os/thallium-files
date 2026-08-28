//! Stable domain identifiers and monotonic version markers.
//!
//! The names follow the protocol vocabulary in `PHASE2-PLAN.md` sections 2 and
//! 5.

/// Stable identity of a directory entry independent of its visible row.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntryId(pub u128);

impl EntryId {
	/// Derives a deterministic filesystem-object identity from backend, device,
	/// and inode.
	///
	/// Device/inode identity keeps the ID stable across a rename. Two fixed
	/// independent 64-bit hashes are combined rather than using Rust's
	/// process-randomized default hasher, making results stable across launches.
	#[must_use]
	pub fn from_file_identity(backend: crate::BackendId, device: u64, inode: u64) -> Self {
		Self::hash_parts(backend, device, inode, &[])
	}

	/// Derives a deterministic directory-entry identity.
	///
	/// Including the path distinguishes multiple hard links to one inode. The ID
	/// is stable across refreshes at the same location; delta reconciliation uses
	/// device/inode to recognize an unambiguous rename.
	#[must_use]
	pub fn from_entry(
		backend: crate::BackendId,
		device: u64,
		inode: u64,
		path: &crate::NativePath,
	) -> Self {
		Self::hash_parts(backend, device, inode, path.as_bytes())
	}

	fn hash_parts(backend: crate::BackendId, device: u64, inode: u64, path: &[u8]) -> Self {
		fn fnv1a(seed: u64, chunks: impl IntoIterator<Item = u8>) -> u64 {
			chunks
				.into_iter()
				.fold(seed, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3))
		}

		let mut bytes = [0; 20];
		bytes[..4].copy_from_slice(&backend.0.to_le_bytes());
		bytes[4..12].copy_from_slice(&device.to_le_bytes());
		bytes[12..].copy_from_slice(&inode.to_le_bytes());
		let high = fnv1a(0xcbf29ce484222325, bytes.iter().chain(path).copied());
		let low = fnv1a(0x84222325cbf29ce4, bytes.iter().chain(path).copied());
		Self((u128::from(high) << 64) | u128::from(low))
	}
}

/// Generation attached to a directory listing or refresh.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ListingGeneration(pub u64);

/// Monotonic revision of renderer-neutral application state.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StateRevision(pub u64);

impl ListingGeneration {
	/// Returns the next generation, saturating instead of wrapping stale work
	/// into the initial generation.
	#[must_use]
	pub const fn next(self) -> Self { Self(self.0.saturating_add(1)) }
}

impl StateRevision {
	/// Returns the next state revision, saturating at the maximum value.
	#[must_use]
	pub const fn next(self) -> Self { Self(self.0.saturating_add(1)) }
}

#[cfg(test)]
mod tests {
	use super::EntryId;
	use crate::BackendId;

	#[test]
	fn entry_identity_is_deterministic_and_uses_all_parts() {
		let id = EntryId::from_file_identity(BackendId::LOCAL, 8, 42);
		assert_eq!(id, EntryId::from_file_identity(BackendId::LOCAL, 8, 42));
		assert_ne!(id, EntryId::from_file_identity(BackendId::LOCAL, 9, 42));
		assert_ne!(id, EntryId::from_file_identity(BackendId(1), 8, 42));
		assert_ne!(
			EntryId::from_entry(BackendId::LOCAL, 8, 42, &crate::NativePath::from(&b"/a"[..])),
			EntryId::from_entry(BackendId::LOCAL, 8, 42, &crate::NativePath::from(&b"/b"[..]))
		);
	}
}
