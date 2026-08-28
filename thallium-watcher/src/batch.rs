//! Notification batching placeholder.
//!
//! The eventual bounded-by-count-and-time policy follows `PHASE2-PLAN.md`
//! section 4. Yazi's 1,000-item/250-ms reference behavior is documented in
//! `WATCHER-STATE-RESEARCH.md` section 1.2.

use std::collections::BTreeSet;

use crate::{NotificationBatch, NotificationHint, WatcherConfig};

/// Collects and deduplicates platform-neutral notification hints.
#[derive(Debug)]
pub struct NotificationBatcher {
	config:  WatcherConfig,
	pending: BTreeSet<NotificationHint>,
}

impl NotificationBatcher {
	/// Construct an empty batcher with explicit bounds.
	#[must_use]
	pub fn new(config: WatcherConfig) -> Self { Self { config, pending: BTreeSet::new() } }

	/// Add a hint and return a complete batch when a configured bound is met.
	pub fn push(&mut self, hint: NotificationHint) -> Option<NotificationBatch> {
		self.pending.insert(hint);
		(self.pending.len() >= self.config.max_batch_size)
			.then(|| self.flush().expect("a size-complete notification batch cannot be empty"))
	}

	/// Close the current batch, if it contains any hints.
	pub fn flush(&mut self) -> Option<NotificationBatch> {
		(!self.pending.is_empty())
			.then(|| NotificationBatch { hints: std::mem::take(&mut self.pending).into_iter().collect() })
	}

	/// Return the batching policy retained by this collector.
	#[must_use]
	pub fn config(&self) -> WatcherConfig { self.config }

	/// Return the number of hints awaiting reconciliation.
	#[must_use]
	pub fn pending_len(&self) -> usize { self.pending.len() }
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use thallium_fs_core::{Location, NativePath};

	use super::NotificationBatcher;
	use crate::{NotificationHint, WatcherConfig};

	fn hint(path: &[u8]) -> NotificationHint {
		NotificationHint { location: Location::local(NativePath::from(path)) }
	}

	#[test]
	fn deduplicates_and_flushes_at_the_unique_hint_limit() {
		let mut batcher = NotificationBatcher::new(WatcherConfig {
			notification_capacity: 4,
			max_batch_size:        2,
			batch_window:          Duration::from_millis(1),
		});
		assert_eq!(batcher.push(hint(b"/tmp/a")), None);
		assert_eq!(batcher.push(hint(b"/tmp/a")), None);
		let batch = batcher.push(hint(b"/tmp/b")).expect("size-complete batch");
		assert_eq!(batch.hints, [hint(b"/tmp/a"), hint(b"/tmp/b")]);
		assert_eq!(batcher.pending_len(), 0);
	}
}
