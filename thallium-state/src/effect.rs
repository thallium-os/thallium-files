//! Owned effect descriptions returned by pure state transitions.
//!
//! Per `PHASE2-PLAN.md` section 4, effects are data for `thallium-actions` to
//! execute. They are not futures, closures, senders, VFS handles, or Qt calls.

use thallium_fs_core::{ConflictPolicy, ListingGeneration, Location};
use thallium_protocol::{ConflictId, TaskKind};

/// Boundary-neutral request to submit aggregate filesystem work.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskRequest {
	/// Semantic operation category.
	pub kind:            TaskKind,
	/// Source roots belonging to one aggregate user action.
	pub sources:         Vec<Location>,
	/// Optional destination for operations that require one.
	pub destination:     Option<Location>,
	/// Conflict behavior requested by the semantic action.
	pub conflict_policy: ConflictPolicy,
}

/// Side-effect intent emitted by a reducer for the action coordinator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StateEffect {
	/// Scan one directory for a specific listing generation.
	ScanDirectory { location: Location, generation: ListingGeneration },
	/// Replace the set of locations observed by the watcher service.
	WatchLocations { locations: Vec<Location> },
	/// Ask the watcher to reconcile current filesystem truth.
	RefreshDirectory { location: Location },
	/// Submit an aggregate task without exposing scheduler internals.
	SubmitTask(TaskRequest),
	/// Surface an already-identified conflict to the action/frontend workflow.
	RequestConflictDecision { conflict_id: ConflictId },
}
