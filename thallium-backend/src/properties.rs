//! What the Properties sheet shows about one entry, and changing its
//! permission bits.

use std::{fs, io, os::unix::fs::{MetadataExt, PermissionsExt}, path::{Path, PathBuf}, time::UNIX_EPOCH};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Properties {
	pub path:        PathBuf,
	pub directory:   bool,
	/// Where a symbolic link points, as stored in the link.
	pub link_target: Option<PathBuf>,
	/// Size of the entry itself; a folder's contents are measured apart.
	pub size:        u64,
	/// Permission bits, `0o7777` at most.
	pub mode:        u32,
	pub owner:       String,
	pub group:       String,
	/// Seconds since the epoch.
	pub modified:    i64,
	pub accessed:    i64,
	pub changed:     i64,
	pub created:     Option<i64>,
	/// Whether this user may change the permission bits.
	pub owned:       bool,
}

/// Read the properties of `path`, describing a link's target but keeping
/// the link's own destination.
pub fn read(path: &Path) -> io::Result<Properties> {
	let own = fs::symlink_metadata(path)?;
	let link_target = own.file_type().is_symlink().then(|| fs::read_link(path).ok()).flatten();
	let metadata = fs::metadata(path).unwrap_or(own);
	Ok(Properties {
		path: path.to_path_buf(),
		directory: metadata.is_dir(),
		link_target,
		size: metadata.len(),
		mode: metadata.mode() & 0o7777,
		owner: name_of("/etc/passwd", metadata.uid()),
		group: name_of("/etc/group", metadata.gid()),
		modified: metadata.mtime(),
		accessed: metadata.atime(),
		changed: metadata.ctime(),
		created: metadata
			.created()
			.ok()
			.and_then(|time| time.duration_since(UNIX_EPOCH).ok())
			.and_then(|since| i64::try_from(since.as_secs()).ok()),
		owned: current_uid().is_some_and(|uid| uid == 0 || uid == metadata.uid()),
	})
}

/// Replace the permission bits of `path`.
pub fn set_mode(path: &Path, mode: u32) -> io::Result<()> {
	fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o7777))
}

/// `rwxr-xr-x` for `0o755`, with the special bits shown where `ls` shows
/// them.
pub fn mode_text(mode: u32) -> String {
	let mut text: Vec<char> = "rwxrwxrwx"
		.chars()
		.enumerate()
		.map(|(at, letter)| if mode & (0o400 >> at) != 0 { letter } else { '-' })
		.collect();
	for (bit, at, set, unset) in [(0o4000, 2, 's', 'S'), (0o2000, 5, 's', 'S'), (0o1000, 8, 't', 'T')]
	{
		if mode & bit != 0 {
			text[at] = if text[at] == 'x' { set } else { unset };
		}
	}
	text.into_iter().collect()
}

/// The name an `/etc/passwd`- or `/etc/group`-style table gives `id`, or
/// the number when it has none (as for a directory-service account).
fn name_of(table: &str, id: u32) -> String {
	fs::read_to_string(table)
		.ok()
		.and_then(|text| {
			text.lines().find_map(|line| {
				let mut fields = line.split(':');
				let name = fields.next()?;
				let number = fields.nth(1)?;
				(number.parse::<u32>().ok()? == id).then(|| name.to_owned())
			})
		})
		.unwrap_or_else(|| id.to_string())
}

/// This process's user id, read from the owner of `/proc/self`.
fn current_uid() -> Option<u32> { fs::metadata("/proc/self").ok().map(|metadata| metadata.uid()) }

#[cfg(test)]
mod tests {
	use std::{fs, os::unix::fs::PermissionsExt};

	use super::{mode_text, read, set_mode};

	#[test]
	fn modes_read_like_ls() {
		assert_eq!(mode_text(0o755), "rwxr-xr-x");
		assert_eq!(mode_text(0o640), "rw-r-----");
		assert_eq!(mode_text(0o4755), "rwsr-xr-x");
		assert_eq!(mode_text(0o1777), "rwxrwxrwt");
		assert_eq!(mode_text(0o2644), "rw-r-Sr--");
	}

	#[test]
	fn reading_and_changing_permissions() {
		let folder = tempfile::tempdir().expect("folder");
		let file = folder.path().join("note.txt");
		fs::write(&file, b"12345").expect("seed");
		set_mode(&file, 0o640).expect("chmod");

		let properties = read(&file).expect("properties");
		assert_eq!(properties.mode, 0o640);
		assert_eq!(properties.size, 5);
		assert!(!properties.directory);
		assert!(properties.owned);
		assert!(!properties.owner.is_empty());
		assert_eq!(fs::metadata(&file).expect("stat").permissions().mode() & 0o777, 0o640);

		let link = folder.path().join("link");
		std::os::unix::fs::symlink("note.txt", &link).expect("link");
		let through = read(&link).expect("link properties");
		assert_eq!(through.link_target.as_deref(), Some("note.txt".as_ref()));
		assert_eq!(through.size, 5, "a link describes what it points at");
	}
}
