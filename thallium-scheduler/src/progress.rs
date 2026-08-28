//! Aggregate progress facts reduced from bounded worker updates.

use thallium_fs_core::Location;
use thallium_protocol::TaskProgress;

/// Renderer-neutral progress for a semantic task and its internal leaves.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AggregateProgress {
	pub discovery_complete: bool,
	pub items_discovered:   u64,
	pub items_completed:    u64,
	pub items_failed:       u64,
	pub items_skipped:      u64,
	pub bytes_discovered:   u64,
	pub bytes_completed:    u64,
	pub active_items:       Vec<ActiveWork>,
}

impl AggregateProgress {
	pub(crate) fn protocol(&self) -> TaskProgress {
		TaskProgress {
			items_completed: self.items_completed,
			items_total:     self.discovery_complete.then_some(self.items_discovered),
			bytes_completed: self.bytes_completed,
			bytes_total:     self.discovery_complete.then_some(self.bytes_discovered),
		}
	}
}

/// One active leaf retained for status presentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveWork {
	pub location: Location,
	pub phase:    WorkPhase,
}

/// Phase of one active leaf operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkPhase {
	Discovering,
	Reading,
	Writing,
	Publishing,
	Removing,
	Cleaning,
}
