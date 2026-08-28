//! Stable scalar identifiers exposed to the frontend.
//!
//! These values are deliberately data-only. Allocation remains the
//! responsibility of the scheduler and action/state loop.

use thallium_fs_core::EntryId;

macro_rules! id_type {
	($(#[$meta:meta])* $name:ident) => {
		$(#[$meta])*
		#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
		pub struct $name(pub u64);

		impl $name {
			/// Creates an identifier from its bridge-safe scalar value.
			#[must_use]
			pub const fn new(value: u64) -> Self { Self(value) }

			/// Returns the bridge-safe scalar value.
			#[must_use]
			pub const fn value(self) -> u64 { self.0 }
		}
	};
}

id_type!(
	/// Identity of one aggregate filesystem task.
	TaskId
);
id_type!(
	/// Monotonic revision of a task snapshot.
	TaskRevision
);
id_type!(
	/// Stable identity retained for the future multi-tab extension.
	TabId
);
id_type!(
	/// Identity of a conflict awaiting a frontend decision.
	ConflictId
);

/// A bridge-safe representation of the filesystem core's 128-bit entry ID.
///
/// CXX has no portable `u128` value, so the identity is split into two `u64`
/// fields without losing information.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntryIdDto {
	pub high: u64,
	pub low:  u64,
}

impl EntryIdDto {
	/// Creates an ID from its two bridge-safe words.
	#[must_use]
	pub const fn new(high: u64, low: u64) -> Self { Self { high, low } }

	/// Reconstructs the filesystem-core ID exactly.
	#[must_use]
	pub const fn to_entry_id(self) -> EntryId {
		EntryId(((self.high as u128) << 64) | self.low as u128)
	}
}

impl From<EntryId> for EntryIdDto {
	fn from(id: EntryId) -> Self { Self::new((id.0 >> 64) as u64, id.0 as u64) }
}

impl From<EntryIdDto> for EntryId {
	fn from(id: EntryIdDto) -> Self { id.to_entry_id() }
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::EntryId;

	use super::EntryIdDto;

	#[test]
	fn entry_id_round_trip_preserves_all_bits() {
		let original = EntryId(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210);
		let dto = EntryIdDto::from(original);

		assert_eq!(dto.high, 0x0123_4567_89ab_cdef);
		assert_eq!(dto.low, 0xfedc_ba98_7654_3210);
		assert_eq!(dto.to_entry_id(), original);
	}
}
