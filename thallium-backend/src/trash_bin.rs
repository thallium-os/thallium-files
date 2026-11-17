//! The desktop trash as a place to look into: what is there, where it came
//! from, and putting it back or letting it go for good.
//!
//! Listing and restoring are quick renames and run on the caller's thread.
//! Purging can walk large trees, so it runs on a thread of its own and is
//! observed through [`Purge`].

use std::{sync::{Arc, Mutex}, thread};

use trash::{TrashItem, os_limited};

/// Everything in the trash, most recently deleted first.
pub fn list() -> Result<Vec<TrashItem>, String> {
	let mut items = os_limited::list().map_err(|error| error.to_string())?;
	items.sort_by_key(|item| std::cmp::Reverse(item.time_deleted));
	Ok(items)
}

/// Put items back where they were deleted from. An item whose original
/// place has since been taken stays in the trash, and is named in the error.
pub fn restore(items: Vec<TrashItem>) -> Result<(), String> {
	os_limited::restore_all(items).map_err(|error| match error {
		trash::Error::RestoreCollision { path, .. } => {
			format!("{} already exists, so it was left in the trash", path.display())
		}
		trash::Error::RestoreTwins { path, .. } => {
			format!("more than one item would go back to {}", path.display())
		}
		other => other.to_string(),
	})
}

/// A purge in flight. `None` while it runs, then its outcome once.
#[derive(Clone, Default)]
pub struct Purge(Arc<Mutex<Option<Result<(), String>>>>);

impl Purge {
	/// Delete `items` from the trash for good, on another thread.
	pub fn start(items: Vec<TrashItem>) -> Self {
		let purge = Self::default();
		let slot = Arc::clone(&purge.0);
		thread::spawn(move || {
			let outcome = os_limited::purge_all(items).map_err(|error| error.to_string());
			if let Ok(mut slot) = slot.lock() {
				*slot = Some(outcome);
			}
		});
		purge
	}

	/// The outcome, once the purge has finished.
	pub fn take(&self) -> Option<Result<(), String>> { self.0.lock().ok()?.take() }
}

#[cfg(test)]
mod tests {
	use std::{fs, path::PathBuf, time::Duration};

	use super::{Purge, list, restore};

	/// A scratch folder on the home filesystem, so trashing uses the home
	/// trash rather than a per-mount one.
	fn scratch(label: &str) -> PathBuf {
		let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
			.join("../target/trash-tests")
			.join(format!("{label}-{}", std::process::id()));
		fs::create_dir_all(&folder).expect("scratch folder");
		// The trash records normalized paths.
		folder.canonicalize().expect("canonical scratch folder")
	}

	#[test]
	fn trashed_files_are_listed_restored_and_purged() {
		let folder = scratch("cycle");
		let file = folder.join("thallium-trash-test.txt");
		fs::write(&file, b"kept").expect("seed");

		trash::delete(&file).expect("trash");
		let found =
			|items: &[trash::TrashItem]| items.iter().find(|item| item.original_path() == file).cloned();
		let item = found(&list().expect("list")).expect("listed after trashing");
		restore(vec![item]).expect("restore");
		assert_eq!(fs::read(&file).expect("restored"), b"kept");

		trash::delete(&file).expect("trash again");
		let item = found(&list().expect("list")).expect("listed again");
		let purge = Purge::start(vec![item]);
		let outcome = loop {
			if let Some(outcome) = purge.take() {
				break outcome;
			}
			std::thread::sleep(Duration::from_millis(10));
		};
		outcome.expect("purge");
		assert!(found(&list().expect("list")).is_none(), "purged items leave the trash");
		let _ = fs::remove_dir_all(folder);
	}

	#[test]
	fn restoring_onto_a_taken_name_keeps_both() {
		let folder = scratch("collision");
		let file = folder.join("thallium-trash-clash.txt");
		fs::write(&file, b"old").expect("seed");
		trash::delete(&file).expect("trash");
		fs::write(&file, b"new").expect("replacement");

		let item =
			list().expect("list").into_iter().find(|item| item.original_path() == file).expect("listed");
		let error = restore(vec![item.clone()]).expect_err("a taken name refuses the restore");
		assert!(error.contains("already exists"), "{error}");
		assert_eq!(fs::read(&file).expect("untouched"), b"new");
		trash::os_limited::purge_all([item]).expect("clean up");
		let _ = fs::remove_dir_all(folder);
	}
}
