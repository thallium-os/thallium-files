//! Backend-qualified watch targets.
//!
//! The local/virtual distinction is adapted from Yazi's watched-target shape
//! documented in `WATCHER-STATE-RESEARCH.md` section 1.1. Virtual observation
//! is only a future seam; the v1 implementation remains local-only.

use thallium_fs_core::Location;

/// A location retained by the watcher service.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Watchee {
	/// A location observed through the platform filesystem watcher.
	Local {
		/// Backend-qualified local location.
		location: Location,
		/// Whether a future backend should use polling for this location.
		polling:  bool,
	},
	/// Placeholder for a future backend that reports changes explicitly.
	Virtual {
		/// Backend-qualified virtual location.
		location: Location,
	},
}

impl Watchee {
	/// Construct a local native watch. Polling is deliberately disabled in the
	/// Linux MVP; `notify::RecommendedWatcher` uses inotify.
	#[must_use]
	pub fn local(location: Location) -> Self {
		Self::Local { location: location.normalize(), polling: false }
	}

	/// Construct a future virtual-backend target. It is retained in the desired
	/// set but has no platform adapter in the MVP.
	#[must_use]
	pub fn virtual_location(location: Location) -> Self { Self::Virtual { location } }

	/// Return the backend-qualified location represented by this target.
	#[must_use]
	pub fn location(&self) -> &Location {
		match self {
			Self::Local { location, .. } | Self::Virtual { location } => location,
		}
	}
}
