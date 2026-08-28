//! Desired and active watcher-set state.
//!
//! This shape follows the coalesced desired-set lifecycle described in
//! `WATCHER-STATE-RESEARCH.md` section 1.1 and remains independent of browser
//! state and rendering.

use std::collections::BTreeSet;

use crate::Watchee;

/// The complete set of locations that should currently be observed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WatchedSet {
	/// Ordered targets used to make synchronization deterministic.
	pub entries: BTreeSet<Watchee>,
}

/// Additions and removals required to reach a desired watch set.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WatchSetDiff {
	/// Targets that must begin being observed.
	pub added:   Vec<Watchee>,
	/// Targets that must stop being observed.
	pub removed: Vec<Watchee>,
}

impl WatchedSet {
	/// Construct a desired set from backend-qualified targets.
	#[must_use]
	pub fn new(entries: impl IntoIterator<Item = Watchee>) -> Self {
		Self { entries: entries.into_iter().collect() }
	}

	/// Construct the MVP's single non-recursive local directory watch.
	#[must_use]
	pub fn single_local(location: thallium_fs_core::Location) -> Self {
		Self::new([Watchee::local(location)])
	}

	/// Compute the operations required to replace this set with `desired`.
	#[must_use]
	pub fn diff(&self, desired: &Self) -> WatchSetDiff {
		WatchSetDiff {
			added:   desired.entries.difference(&self.entries).cloned().collect(),
			removed: self.entries.difference(&desired.entries).cloned().collect(),
		}
	}
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::{Location, NativePath};

	use super::WatchedSet;
	use crate::Watchee;

	fn local(path: &[u8]) -> Watchee { Watchee::local(Location::local(NativePath::from(path))) }

	#[test]
	fn diff_is_deterministic() {
		let current = WatchedSet::new([local(b"/b"), local(b"/a")]);
		let desired = WatchedSet::new([local(b"/b"), local(b"/c")]);
		let diff = current.diff(&desired);
		assert_eq!(diff.added, [local(b"/c")]);
		assert_eq!(diff.removed, [local(b"/a")]);
	}
}
