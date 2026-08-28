//! Renderer-independent navigation history for the single MVP browser pane.
//!
//! This extracts navigation data from the monolithic Yazi tab shape described
//! in `WATCHER-STATE-RESEARCH.md` section 2.3 without retaining tab machinery.

use thallium_fs_core::Location;

const MAX_HISTORY: usize = 128;

/// Locations available for backward and forward navigation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NavigationHistory {
	/// Locations available to navigate backward to, newest last.
	pub back:    Vec<Location>,
	/// Locations available to navigate forward to, newest last.
	pub forward: Vec<Location>,
}

impl NavigationHistory {
	/// Returns whether backward navigation is available.
	#[must_use]
	pub fn can_go_back(&self) -> bool { !self.back.is_empty() }

	/// Returns whether forward navigation is available.
	#[must_use]
	pub fn can_go_forward(&self) -> bool { !self.forward.is_empty() }

	pub(crate) fn push_back(&mut self, location: Location) { push_bounded(&mut self.back, location) }

	pub(crate) fn push_forward(&mut self, location: Location) {
		push_bounded(&mut self.forward, location);
	}
}

fn push_bounded(stack: &mut Vec<Location>, location: Location) {
	if stack.len() == MAX_HISTORY {
		stack.remove(0);
	}
	stack.push(location);
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::{Location, NativePath};

	use super::{MAX_HISTORY, NavigationHistory};

	#[test]
	fn history_discards_the_oldest_location_at_its_bound() {
		let mut history = NavigationHistory::default();
		for index in 0..=MAX_HISTORY {
			history.push_back(Location::local(NativePath::from(format!("/{index}").into_bytes())));
		}

		assert_eq!(history.back.len(), MAX_HISTORY);
		assert_eq!(history.back.first().expect("first retained").path.as_bytes(), b"/1");
		assert_eq!(
			history.back.last().expect("last retained").path.as_bytes(),
			format!("/{MAX_HISTORY}").as_bytes()
		);
	}
}
