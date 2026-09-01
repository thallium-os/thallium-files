//! Icons and thumbnails for directory entries.
//!
//! Two answers per entry, both cheap to ask for repeatedly because both are
//! cached: the type icon from the user's icon theme and the thumbnail. Both
//! are resolved on workers and answer immediately, so a directory full of
//! types neither cache has seen never holds up the thread that draws.
//!
//! Nothing here knows about Qt. The service hands back filesystem paths; how
//! they are loaded and tinted is the frontend's business.

mod mime;
mod theme;
mod thumbnail;

use std::path::{Path, PathBuf};

pub use crate::thumbnail::Thumbnail;
use crate::{mime::MimeIndex, theme::IconTheme, thumbnail::ThumbnailStore};

/// The size icons are looked up at. Themes are indexed by size, so this picks
/// which directory of the theme is searched, not how the icon is drawn: a
/// scalable SVG is returned whatever the number, and the frontend scales it.
const ICON_SIZE: u16 = 48;

pub struct IconService {
	mime:       MimeIndex,
	theme:      IconTheme,
	thumbnails: ThumbnailStore,
}

impl IconService {
	pub fn new() -> Self {
		Self {
			mime:       MimeIndex::new(),
			theme:      IconTheme::new(ICON_SIZE),
			thumbnails: ThumbnailStore::new(),
		}
	}

	/// The icon theme being searched, for diagnostics.
	pub fn theme_name(&self) -> &str { self.theme.name() }

	/// The MIME type of `path`, which the caller also passes back to
	/// [`Self::thumbnail`] rather than having it guessed twice.
	pub fn mime_of(&self, path: &Path, is_directory: bool) -> String {
		self.mime.of(path, is_directory)
	}

	/// The type icon for a MIME type, or `None` when no installed theme has one.
	pub fn icon(&self, mime: &str) -> Option<PathBuf> {
		self.theme.find(mime, || self.mime.icon_names(mime))
	}

	/// The thumbnail for `path`, queueing generation the first time it is asked
	/// for. Returns immediately in every case.
	pub fn thumbnail(&self, path: &Path, mime: &str, modified: i64) -> Thumbnail {
		self.thumbnails.request(path, mime, modified)
	}

	/// Whether any queued thumbnail or icon search finished since the last
	/// call, meaning the view has something new to show.
	pub fn poll_thumbnails(&self) -> bool {
		// Both are asked, never short-circuited: each clears its own counter.
		let thumbnails = self.thumbnails.take_completed() > 0;
		let icons = self.theme.take_completed() > 0;
		thumbnails || icons
	}
}

/// A `file://` URL for `path`, percent-encoded so a name with a space or a
/// `#` in it still loads.
#[must_use]
pub fn file_url(path: &Path) -> String { crate::thumbnail::file_uri(path) }

impl Default for IconService {
	fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::IconService;

	#[test]
	fn directories_are_typed_without_touching_the_filesystem() {
		let service = IconService::new();
		assert_eq!(service.mime_of(Path::new("/nonexistent/place"), true), "inode/directory");
	}

	#[test]
	fn a_rust_file_is_typed_from_its_name_alone() {
		let service = IconService::new();
		// The path does not exist; only the suffix is consulted.
		let mime = service.mime_of(Path::new("/nonexistent/main.rs"), false);
		assert!(mime.starts_with("text/"), "unexpected mime {mime}");
	}

	#[test]
	fn nothing_is_pending_before_anything_is_asked_for() {
		assert!(!IconService::new().poll_thumbnails());
	}
}
