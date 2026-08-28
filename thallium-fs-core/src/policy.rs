//! User-visible conflict policies for filesystem mutations.
//!
//! The policy is owned data so actions can suspend for a decision without
//! retaining a closure, as required by `PHASE2-PLAN.md` sections 3 and 5.

/// Policy applied when an operation encounters an existing destination.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ConflictPolicy {
	/// Suspend and request a decision from the action/frontend boundary.
	#[default]
	Ask,
	/// Replace the existing destination when the operation permits it.
	Overwrite,
	/// Leave the existing destination and skip this item.
	Skip,
	/// Select a non-conflicting destination name.
	Rename,
	/// Abort the aggregate operation.
	Abort,
}
