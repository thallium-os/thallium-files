//! Private ownership graph for long-lived backend services.
//!
//! This is the composition described by `PHASE2-PLAN.md` section 1. Keeping it
//! private prevents scheduler handles, VFS objects, watcher state, and mutable
//! browser state from becoming part of the Qt-facing contract.

use thallium_actions::{Command, CommandEnvelope};
use thallium_protocol::{StateDelta, StateSnapshot};
use tokio::sync::oneshot;

/// Services owned and shut down together by [`crate::Backend`].
///
/// The coordinator is the single owner of browser state and of the VFS,
/// scheduler, and watcher effect services. The backend owns that complete
/// graph without duplicating mutable ownership.
pub(super) enum ServiceRequest {
	Dispatch(CommandEnvelope, oneshot::Sender<Result<(), String>>),
	DispatchCurrent(Command, oneshot::Sender<Result<(), String>>),
	Snapshot(oneshot::Sender<StateSnapshot>),
	Shutdown(oneshot::Sender<Result<(), String>>),
}

pub(super) fn publish(deltas: Vec<StateDelta>, events: &tokio::sync::mpsc::Sender<StateDelta>) {
	for delta in deltas {
		// Deltas are advisory; a slow frontend recovers from the authoritative
		// snapshot. Admission remains bounded and the state owner never blocks.
		let _ = events.try_send(delta);
	}
}
