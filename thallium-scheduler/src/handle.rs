//! Rust-internal task control and observation handle.

use std::sync::{Arc, atomic::{AtomicU8, Ordering}};

use thallium_protocol::{TaskId, TaskInfo, TaskStatus};
use tokio::sync::{Notify, watch};
use tokio_util::sync::CancellationToken;

/// Cloneable in-process handle for one aggregate task.
#[derive(Clone)]
pub struct TaskHandle {
	pub(crate) id:           TaskId,
	pub(crate) cancellation: CancellationToken,
	pub(crate) snapshots:    watch::Receiver<TaskInfo>,
	pub(crate) lifecycle:    Arc<AtomicU8>,
	pub(crate) completion:   Arc<Notify>,
}

impl TaskHandle {
	/// Return the task's stable identity.
	#[must_use]
	pub const fn id(&self) -> TaskId { self.id }

	/// Request cooperative cancellation.
	///
	/// The scheduler-owned reducer publishes the resulting terminal snapshot.
	pub fn cancel(&self) {
		self.cancellation.cancel();
		let current = self.lifecycle.load(Ordering::Acquire);
		if current < 3 {
			let _ = self.lifecycle.compare_exchange(current, 3, Ordering::AcqRel, Ordering::Acquire);
		}
	}

	/// Clone the latest owned boundary-safe snapshot.
	#[must_use]
	pub fn snapshot(&self) -> TaskInfo { self.snapshots.borrow().clone() }

	/// Wait until a newer snapshot is published.
	// Both variants carry the same snapshot: the Err arm reports that the
	// publisher is gone, not a different payload. Boxing one side would grow
	// the type rather than shrink it.
	#[allow(clippy::result_large_err)]
	pub async fn changed(&mut self) -> Result<TaskInfo, TaskInfo> {
		match self.snapshots.changed().await {
			Ok(()) => Ok(self.snapshot()),
			Err(_) => Err(self.snapshot()),
		}
	}

	/// Wait for and return the terminal task snapshot.
	pub async fn wait(&mut self) -> TaskInfo {
		loop {
			let snapshot = self.snapshot();
			if snapshot.status.is_terminal() {
				return snapshot;
			}
			let notified = self.completion.notified();
			let snapshot = self.snapshot();
			if snapshot.status.is_terminal() {
				return snapshot;
			}
			notified.await;
		}
	}

	/// Read the lifecycle without locking or cloning a snapshot.
	#[must_use]
	pub fn status(&self) -> TaskStatus {
		match self.lifecycle.load(Ordering::Acquire) {
			0 => TaskStatus::Pending,
			1 | 2 => TaskStatus::Started,
			3 => TaskStatus::Cancelling,
			4 => TaskStatus::Succeeded,
			5 => TaskStatus::Failed,
			_ => TaskStatus::Cancelled,
		}
	}
}
