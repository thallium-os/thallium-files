//! Owned notification hints and batching configuration.
//!
//! OS notifications are represented only as invalidation hints; authoritative
//! changes are derived by restatting through VFS, as selected in
//! `PHASE2-PLAN.md` section 4 and informed by
//! `WATCHER-STATE-RESEARCH.md` section 1.2.

use std::time::Duration;

use thallium_fs_core::{FsError, FsErrorCode, FsResult, Location};

/// A platform-independent hint that a location may have changed.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NotificationHint {
	/// Location to restat during reconciliation.
	pub location: Location,
}

/// A bounded group of notification hints ready for reconciliation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NotificationBatch {
	/// Owned hints collected for this batch.
	pub hints: Vec<NotificationHint>,
}

/// Runtime policy for bounded notification ingestion and reconciliation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatcherConfig {
	/// Capacity of the internal notification queue.
	pub notification_capacity: usize,
	/// Maximum hints accepted into one batch before it closes.
	pub max_batch_size:        usize,
	/// Maximum time an incomplete batch waits for another hint.
	pub batch_window:          Duration,
}

impl WatcherConfig {
	/// Linux MVP defaults: a bounded 1,000-hint queue and batch, with a fixed
	/// 250-ms debounce window measured from the first admitted hint.
	pub const DEFAULT: Self = Self {
		notification_capacity: 1_000,
		max_batch_size:        1_000,
		batch_window:          Duration::from_millis(250),
	};

	/// Reject configurations that would make Tokio's bounded queue panic or
	/// prevent the batcher from ever making progress.
	pub fn validate(self) -> FsResult<Self> {
		if self.notification_capacity == 0 {
			return Err(FsError::new(
				FsErrorCode::InvalidInput,
				"watcher notification capacity must be greater than zero",
				None,
			));
		}
		if self.max_batch_size == 0 {
			return Err(FsError::new(
				FsErrorCode::InvalidInput,
				"watcher batch size must be greater than zero",
				None,
			));
		}
		Ok(self)
	}
}

impl Default for WatcherConfig {
	fn default() -> Self { Self::DEFAULT }
}
