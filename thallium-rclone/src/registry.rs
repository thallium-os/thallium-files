//! The mapping between rclone remote names and backend identities.
//!
//! `BackendId(0)` is the local filesystem and is never issued here. Every
//! configured remote gets the next identity in listing order, so a location
//! carries which service it belongs to without carrying its name.

use std::sync::{Arc, RwLock};

use thallium_fs_core::{BackendId, FsError, FsErrorCode, FsResult, Location, NativePath};

/// One configured rclone remote.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Remote {
	pub id:   BackendId,
	/// The name as the user configured it, without the trailing colon.
	pub name: String,
}

impl Remote {
	/// The `fs` argument rclone expects, for example `gdrive:`.
	#[must_use]
	pub fn fs(&self) -> String { format!("{}:", self.name) }
}

/// Every remote this session knows about.
///
/// Shared rather than copied: a remote configured from the interface has to
/// become reachable to the router that is already running, without rebuilding
/// the service graph around it.
#[derive(Clone, Debug, Default)]
pub struct Remotes {
	entries: Arc<RwLock<Vec<Remote>>>,
}

impl Remotes {
	/// Assign identities to remote names in the order rclone reported them.
	#[must_use]
	pub fn new(names: Vec<String>) -> Self {
		let remotes = Self::default();
		remotes.replace(names);
		remotes
	}

	/// Take up a fresh listing, keeping the identity of every remote that was
	/// already known.
	///
	/// Renumbering would invalidate locations the browser is already holding —
	/// history, selection, a pending task — so existing names keep their
	/// identity and only new names take new ones.
	pub fn replace(&self, names: Vec<String>) {
		let Ok(mut entries) = self.entries.write() else { return };
		let mut next = entries.iter().map(|remote| remote.id.0).max().unwrap_or(0);
		let mut updated = Vec::with_capacity(names.len());
		for name in names {
			let id = entries.iter().find(|remote| remote.name == name).map_or_else(
				|| {
					next = next.saturating_add(1);
					BackendId(next)
				},
				|remote| remote.id,
			);
			updated.push(Remote { id, name });
		}
		*entries = updated;
	}

	#[must_use]
	pub fn all(&self) -> Vec<Remote> {
		self.entries.read().map(|entries| entries.clone()).unwrap_or_default()
	}

	#[must_use]
	pub fn len(&self) -> usize { self.entries.read().map_or(0, |entries| entries.len()) }

	#[must_use]
	pub fn is_empty(&self) -> bool { self.len() == 0 }

	#[must_use]
	pub fn by_id(&self, id: BackendId) -> Option<Remote> {
		let entries = self.entries.read().ok()?;
		entries.iter().find(|remote| remote.id == id).cloned()
	}

	#[must_use]
	pub fn by_name(&self, name: &str) -> Option<Remote> {
		let entries = self.entries.read().ok()?;
		entries.iter().find(|remote| remote.name == name).cloned()
	}

	#[must_use]
	pub fn at(&self, index: usize) -> Option<Remote> {
		let entries = self.entries.read().ok()?;
		entries.get(index).cloned()
	}

	/// Resolve the remote a location belongs to.
	pub fn resolve(&self, location: &Location) -> FsResult<Remote> {
		self.by_id(location.backend).ok_or_else(|| {
			FsError::new(
				FsErrorCode::Unsupported,
				format!("backend {} is not a configured remote", location.backend.0),
				Some(location.clone()),
			)
		})
	}

	/// Read `name:/path` as written by the frontend.
	///
	/// A string with no configured remote before its colon is a local path:
	/// a Linux file name may legitimately contain a colon, and guessing
	/// otherwise would make such a file unreachable.
	#[must_use]
	pub fn parse_display(&self, text: &str) -> Location {
		if let Some((name, rest)) = text.split_once(':')
			&& let Some(remote) = self.by_name(name)
		{
			let path = if rest.starts_with('/') { rest.to_owned() } else { format!("/{rest}") };
			return Location::new(remote.id, NativePath::new(path.into_bytes())).normalize();
		}
		Location::local(NativePath::new(text.as_bytes().to_vec())).normalize()
	}

	/// Render a location the way [`Self::parse_display`] reads it back.
	#[must_use]
	pub fn display(&self, location: &Location) -> String {
		let path = String::from_utf8_lossy(location.path.as_bytes()).into_owned();
		match self.by_id(location.backend) {
			Some(remote) => format!("{}:{path}", remote.name),
			None => path,
		}
	}
}

/// The path rclone wants for a location: relative, with no leading separator.
#[must_use]
pub fn remote_path(location: &Location) -> String {
	String::from_utf8_lossy(location.path.as_bytes()).trim_start_matches('/').to_owned()
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::{BackendId, Location, NativePath};

	use super::{Remotes, remote_path};

	fn remotes() -> Remotes { Remotes::new(vec!["gdrive".to_owned(), "onedrive".to_owned()]) }

	#[test]
	fn a_new_listing_keeps_the_identities_it_already_issued() {
		let remotes = remotes();
		// onedrive removed, gdrive kept, box added.
		remotes.replace(vec!["gdrive".to_owned(), "box".to_owned()]);
		assert_eq!(remotes.by_name("gdrive").map(|remote| remote.id), Some(BackendId(1)));
		// 2 belonged to onedrive and is not handed out again.
		assert_eq!(remotes.by_name("box").map(|remote| remote.id), Some(BackendId(3)));
		assert_eq!(remotes.len(), 2);
	}

	#[test]
	fn identities_start_after_the_local_filesystem() {
		let remotes = remotes();
		assert_eq!(remotes.by_name("gdrive").map(|remote| remote.id), Some(BackendId(1)));
		assert_eq!(remotes.by_name("onedrive").map(|remote| remote.id), Some(BackendId(2)));
		assert_eq!(remotes.by_id(BackendId::LOCAL), None);
	}

	#[test]
	fn display_round_trips_through_parsing() {
		let remotes = remotes();
		let location = remotes.parse_display("gdrive:/photos/2024");
		assert_eq!(location.backend, BackendId(1));
		assert_eq!(location.path.as_bytes(), b"/photos/2024");
		assert_eq!(remotes.display(&location), "gdrive:/photos/2024");
	}

	#[test]
	fn a_missing_leading_separator_is_supplied() {
		assert_eq!(remotes().parse_display("gdrive:photos").path.as_bytes(), b"/photos");
	}

	#[test]
	fn a_local_file_may_contain_a_colon() {
		let location = remotes().parse_display("/home/ada/2026-05-20,11:17:44.png");
		assert_eq!(location.backend, BackendId::LOCAL);
		assert_eq!(location.path.as_bytes(), b"/home/ada/2026-05-20,11:17:44.png");
	}

	#[test]
	fn rclone_paths_carry_no_leading_separator() {
		let location = Location::new(BackendId(1), NativePath::new(b"/photos/2024".to_vec()));
		assert_eq!(remote_path(&location), "photos/2024");
		let root = Location::new(BackendId(1), NativePath::new(b"/".to_vec()));
		assert_eq!(remote_path(&root), "");
	}
}
