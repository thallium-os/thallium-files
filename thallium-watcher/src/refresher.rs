//! Coalesced directory refresh state.
//!
//! `busy + dirty`, sticky force requests, and generation checks adapt the
//! directly observed `Refresher::Entry` state machine documented in
//! `WATCHER-STATE-RESEARCH.md` section 1.3. No mutable browser state is held.

use std::{collections::BTreeMap, sync::Arc};

use thallium_fs_core::{FileEntry, FsResult, ListingGeneration, Location};
use thallium_vfs::FileSystem;

use crate::WatchedSet;

/// Current scheduling state for a retained directory refresh.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RefreshState {
	/// No directory read is currently in flight.
	#[default]
	Idle,
	/// A read is active; later requests are coalesced into its follow-up state.
	Busy {
		/// Generation assigned to the active read.
		generation: ListingGeneration,
		/// Whether another read must run after the active one completes.
		dirty:      bool,
		/// Whether the coalesced follow-up must bypass metadata revalidation.
		force:      bool,
	},
}

/// Refresh bookkeeping retained for one watched directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshEntry {
	/// Directory whose listing is reconciled.
	pub location:        Location,
	/// Coalescing and stale-completion state.
	pub state:           RefreshState,
	/// Generation to assign to the next read started for this location.
	pub next_generation: ListingGeneration,
}

/// Request to reconcile a complete directory snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshRequest {
	/// Directory to scan or revalidate.
	pub location:      Location,
	/// Bypass metadata short-circuiting and force a complete scan.
	pub force:         bool,
	/// Surface read failures as owned deltas rather than diagnostics only.
	pub report_errors: bool,
}

/// Generation-tagged result returned by a refresh operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshResult {
	/// Directory that was read.
	pub location:   Location,
	/// Generation assigned when the read began.
	pub generation: ListingGeneration,
	/// Complete observed entries or an owned filesystem failure.
	pub entries:    FsResult<Vec<FileEntry>>,
}

/// Owns per-directory refresh generations and coalescing state.
#[derive(Debug)]
pub struct Refresher<B: FileSystem> {
	backend: Arc<B>,
	entries: BTreeMap<Location, RefreshEntry>,
}

impl<B: FileSystem> Refresher<B> {
	/// Construct a refresher over a renderer-neutral filesystem backend.
	#[must_use]
	pub fn new(backend: Arc<B>) -> Self { Self { backend, entries: BTreeMap::new() } }

	/// Retain refresh state only for the current desired watch set.
	pub fn sync(&mut self, watched: &WatchedSet) {
		self
			.entries
			.retain(|location, _| watched.entries.iter().any(|watchee| watchee.location() == location));
		for watchee in &watched.entries {
			self.entries.entry(watchee.location().clone()).or_insert_with(|| RefreshEntry {
				location:        watchee.location().clone(),
				state:           RefreshState::Idle,
				next_generation: ListingGeneration(1),
			});
		}
	}

	/// Coalesce or begin a directory refresh request.
	pub fn request(&mut self, request: RefreshRequest) -> Option<ListingGeneration> {
		let entry = self.entries.entry(request.location.clone()).or_insert_with(|| RefreshEntry {
			location:        request.location,
			state:           RefreshState::Idle,
			next_generation: ListingGeneration(1),
		});
		match &mut entry.state {
			state @ RefreshState::Idle => {
				let generation = entry.next_generation;
				entry.next_generation = generation.next();
				*state = RefreshState::Busy { generation, dirty: false, force: request.force };
				Some(generation)
			}
			RefreshState::Busy { dirty, force, .. } => {
				*dirty = true;
				*force |= request.force;
				None
			}
		}
	}

	/// Read a requested directory without borrowing refresh bookkeeping.
	pub async fn refresh(&self, request: RefreshRequest) -> RefreshResult {
		let generation = self
			.entries
			.get(&request.location)
			.and_then(|entry| match entry.state {
				RefreshState::Busy { generation, .. } => Some(generation),
				RefreshState::Idle => None,
			})
			.unwrap_or_default();
		let entries = self.backend.scan(&request.location).await;
		RefreshResult { location: request.location, generation, entries }
	}

	/// Accept a matching completion and describe whether a follow-up is needed.
	pub fn complete(&mut self, result: &RefreshResult) -> Option<RefreshRequest> {
		let entry = self.entries.get_mut(&result.location)?;
		let RefreshState::Busy { generation, dirty, force } = entry.state else {
			return None;
		};
		if generation != result.generation {
			return None;
		}
		if !dirty {
			entry.state = RefreshState::Idle;
			return None;
		}

		let generation = entry.next_generation;
		entry.next_generation = generation.next();
		entry.state = RefreshState::Busy { generation, dirty: false, force };
		Some(RefreshRequest { location: result.location.clone(), force, report_errors: true })
	}

	/// Return the backend retained by the refresh service.
	#[must_use]
	pub fn backend(&self) -> &Arc<B> { &self.backend }

	/// Return refresh entries for recovery snapshots and diagnostics.
	#[must_use]
	pub fn entries(&self) -> &BTreeMap<Location, RefreshEntry> { &self.entries }
}

#[cfg(test)]
mod tests {
	use std::sync::Arc;

	use thallium_fs_core::{Location, NativePath};
	use thallium_vfs::LocalBackend;

	use super::{RefreshRequest, RefreshState, Refresher};

	fn request(location: &Location, force: bool) -> RefreshRequest {
		RefreshRequest { location: location.clone(), force, report_errors: true }
	}

	#[test]
	fn coalesces_busy_requests_and_keeps_force_sticky() {
		let location = Location::local(NativePath::from(&b"/tmp"[..]));
		let mut refresher = Refresher::new(Arc::new(LocalBackend::new()));
		let first = refresher.request(request(&location, false)).expect("first generation");
		assert_eq!(refresher.request(request(&location, true)), None);
		let result = super::RefreshResult {
			location:   location.clone(),
			generation: first,
			entries:    Ok(Vec::new()),
		};
		let follow_up = refresher.complete(&result).expect("dirty follow-up");
		assert!(follow_up.force);
		assert!(matches!(
			refresher.entries()[&location].state,
			RefreshState::Busy { generation, dirty: false, force: true } if generation.0 == 2
		));
	}
}
