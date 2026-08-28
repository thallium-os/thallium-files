//! Bounded notification reporting endpoint.
//!
//! Yazi's reporter uses an unbounded Tokio channel
//! (`WATCHER-STATE-RESEARCH.md` section 1.2). This stub instead encodes the
//! bounded-queue invariant chosen in `PHASE2-PLAN.md` sections 1 and 4.

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use tokio::sync::mpsc;

use crate::NotificationHint;

/// Failure to admit a notification hint into the bounded watcher queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportError {
	/// The queue is at capacity and cannot accept another hint immediately.
	Full,
	/// The watcher service has closed its receiving endpoint.
	Closed,
}

/// Cloneable producer used by platform adapters and completed operations.
#[derive(Clone, Debug)]
pub struct Reporter {
	sender:     mpsc::Sender<NotificationHint>,
	overflowed: Arc<AtomicBool>,
}

impl Reporter {
	/// Wrap the sending half of a bounded notification channel.
	#[must_use]
	pub fn new(sender: mpsc::Sender<NotificationHint>) -> Self {
		Self { sender, overflowed: Arc::new(AtomicBool::new(false)) }
	}

	/// Report a change hint without blocking the platform callback.
	pub fn report(&self, hint: NotificationHint) -> Result<(), ReportError> {
		self.sender.try_send(hint).map_err(|error| match error {
			mpsc::error::TrySendError::Full(_) => {
				self.overflowed.store(true, Ordering::Release);
				ReportError::Full
			}
			mpsc::error::TrySendError::Closed(_) => ReportError::Closed,
		})
	}

	/// Consume the overflow marker set when at least one hint could not enter the
	/// bounded queue. The watcher responds by scanning every local root.
	pub(crate) fn take_overflowed(&self) -> bool { self.overflowed.swap(false, Ordering::AcqRel) }

	/// Return whether the watcher service has closed its receiver.
	#[must_use]
	pub fn is_closed(&self) -> bool { self.sender.is_closed() }
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::{Location, NativePath};
	use tokio::sync::mpsc;

	use super::{ReportError, Reporter};
	use crate::NotificationHint;

	#[test]
	fn reports_full_and_closed_without_blocking() {
		let (sender, mut receiver) = mpsc::channel(1);
		let reporter = Reporter::new(sender);
		let hint = NotificationHint { location: Location::local(NativePath::from(&b"/tmp/a"[..])) };
		assert_eq!(reporter.report(hint.clone()), Ok(()));
		assert_eq!(reporter.report(hint.clone()), Err(ReportError::Full));
		assert!(reporter.take_overflowed());
		assert!(!reporter.take_overflowed());
		assert_eq!(receiver.try_recv(), Ok(hint.clone()));
		receiver.close();
		assert_eq!(reporter.report(hint), Err(ReportError::Closed));
	}
}
