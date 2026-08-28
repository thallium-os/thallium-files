//! Filesystem entry and metadata snapshots.
//!
//! Field selection adapts Yazi's `File`, `FileExtra`, and `Cha` concepts
//! described in `FS-VFS-RESEARCH.md` section 1; no Yazi representation is
//! reused.

use std::{fs::Metadata, os::unix::fs::{FileTypeExt, MetadataExt}, time::{Duration, SystemTime, UNIX_EPOCH}};

use crate::{EntryId, Location, NativePath};

/// Portable classification of a Linux directory entry.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileKind {
	/// Regular file.
	File,
	/// Directory.
	Directory,
	/// Symbolic link, when observing the link itself.
	Symlink,
	/// Block device.
	BlockDevice,
	/// Character device.
	CharacterDevice,
	/// Named pipe.
	Fifo,
	/// Unix-domain socket.
	Socket,
	/// A kind not represented by the current contract.
	Unknown,
}

/// Timestamp split into Unix seconds and sub-second nanoseconds.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Timestamp {
	/// Whole seconds relative to the Unix epoch.
	pub seconds:     i64,
	/// Nanoseconds within the represented second.
	pub nanoseconds: u32,
}

impl Timestamp {
	/// Creates a normalized timestamp from Unix seconds and nanoseconds.
	#[must_use]
	pub const fn new(seconds: i64, nanoseconds: u32) -> Self { Self { seconds, nanoseconds } }

	/// Converts a standard system time, including times before the Unix epoch.
	#[must_use]
	pub fn from_system_time(time: SystemTime) -> Self {
		match time.duration_since(UNIX_EPOCH) {
			Ok(duration) => Self::from_duration(duration),
			Err(error) => {
				let duration = error.duration();
				if duration.subsec_nanos() == 0 {
					Self::new(-(duration.as_secs().min(i64::MAX as u64) as i64), 0)
				} else {
					let seconds = duration.as_secs().saturating_add(1).min(i64::MAX as u64) as i64;
					Self::new(-seconds, 1_000_000_000 - duration.subsec_nanos())
				}
			}
		}
	}

	fn from_duration(duration: Duration) -> Self {
		Self::new(duration.as_secs().min(i64::MAX as u64) as i64, duration.subsec_nanos())
	}
}

/// Metadata observed for either a directory entry or a followed target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryMetadata {
	/// Entry kind for this metadata observation.
	pub kind:        FileKind,
	/// Logical content length in bytes.
	pub size:        u64,
	/// Linux permission and type mode bits.
	pub permissions: u32,
	/// Owning user identifier.
	pub uid:         u32,
	/// Owning group identifier.
	pub gid:         u32,
	/// Last access time, when supplied by the backend.
	pub accessed:    Option<Timestamp>,
	/// Creation/birth time, when supplied by the backend.
	pub created:     Option<Timestamp>,
	/// Last metadata-change time, when supplied by the backend.
	pub changed:     Option<Timestamp>,
	/// Last content-modification time, when supplied by the backend.
	pub modified:    Option<Timestamp>,
	/// Filesystem device identifier.
	pub device:      u64,
	/// Filesystem inode identifier.
	pub inode:       u64,
	/// Number of hard links.
	pub link_count:  u64,
}

impl EntryMetadata {
	/// Builds a renderer-neutral metadata snapshot from Linux metadata.
	///
	/// Callers choose `std::fs::metadata` or `std::fs::symlink_metadata`, so
	/// following policy remains explicit at the VFS boundary.
	#[must_use]
	pub fn from_std(metadata: &Metadata) -> Self {
		let file_type = metadata.file_type();
		let kind = if file_type.is_file() {
			FileKind::File
		} else if file_type.is_dir() {
			FileKind::Directory
		} else if file_type.is_symlink() {
			FileKind::Symlink
		} else if file_type.is_block_device() {
			FileKind::BlockDevice
		} else if file_type.is_char_device() {
			FileKind::CharacterDevice
		} else if file_type.is_fifo() {
			FileKind::Fifo
		} else if file_type.is_socket() {
			FileKind::Socket
		} else {
			FileKind::Unknown
		};

		Self {
			kind,
			size: metadata.len(),
			permissions: metadata.mode(),
			uid: metadata.uid(),
			gid: metadata.gid(),
			accessed: Some(Timestamp::new(metadata.atime(), metadata.atime_nsec() as u32)),
			created: metadata.created().ok().map(Timestamp::from_system_time),
			changed: Some(Timestamp::new(metadata.ctime(), metadata.ctime_nsec() as u32)),
			modified: Some(Timestamp::new(metadata.mtime(), metadata.mtime_nsec() as u32)),
			device: metadata.dev(),
			inode: metadata.ino(),
			link_count: metadata.nlink(),
		}
	}

	/// Returns a stable filesystem-object ID independent of pathname.
	#[must_use]
	pub fn entry_id(&self, backend: crate::BackendId) -> EntryId {
		EntryId::from_file_identity(backend, self.device, self.inode)
	}

	/// Returns a stable directory-entry ID, distinguishing hard links by path.
	#[must_use]
	pub fn entry_id_at(&self, location: &Location) -> EntryId {
		EntryId::from_entry(location.backend, self.device, self.inode, &location.path)
	}
}

/// Optional details retained separately when an entry is a symbolic link.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SymlinkInfo {
	/// Raw target stored in the link, which may be relative.
	pub target:          NativePath,
	/// Separately observed target metadata, if following succeeded.
	pub target_metadata: Option<EntryMetadata>,
}

/// Owned renderer-neutral directory entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileEntry {
	/// Stable entry identity.
	pub id:       EntryId,
	/// Full backend-qualified location.
	pub location: Location,
	/// Exact final path component.
	pub name:     NativePath,
	/// Non-following metadata for the entry itself.
	pub metadata: EntryMetadata,
	/// Link details kept apart from the entry metadata.
	pub symlink:  Option<SymlinkInfo>,
}

impl FileEntry {
	/// Creates an entry from an explicit location and already-observed metadata.
	#[must_use]
	pub fn new(location: Location, metadata: EntryMetadata, symlink: Option<SymlinkInfo>) -> Self {
		let name = location.path.file_name().unwrap_or_default();
		let id = metadata.entry_id_at(&location);
		Self { id, location, name, metadata, symlink }
	}

	/// Creates an entry when the caller has a separately determined stable ID.
	#[must_use]
	pub fn with_id(
		id: EntryId,
		location: Location,
		metadata: EntryMetadata,
		symlink: Option<SymlinkInfo>,
	) -> Self {
		let name = location.path.file_name().unwrap_or_default();
		Self { id, location, name, metadata, symlink }
	}
}

#[cfg(test)]
mod tests {
	use std::{fs, os::unix::fs::PermissionsExt};

	use super::{EntryMetadata, FileKind};

	#[test]
	fn converts_standard_metadata() {
		let directory = tempfile_dir("metadata");
		let path = directory.join("file");
		fs::write(&path, b"hello").expect("write fixture");
		fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).expect("set mode");

		let metadata = EntryMetadata::from_std(&fs::symlink_metadata(&path).expect("metadata"));
		assert_eq!(metadata.kind, FileKind::File);
		assert_eq!(metadata.size, 5);
		assert_eq!(metadata.permissions & 0o777, 0o640);
		assert_ne!(metadata.inode, 0);

		fs::remove_dir_all(directory).expect("remove fixture");
	}

	fn tempfile_dir(label: &str) -> std::path::PathBuf {
		let path = std::env::temp_dir().join(format!(
			"thallium-fs-core-{label}-{}-{}",
			std::process::id(),
			std::thread::current().name().unwrap_or("test")
		));
		let _ = fs::remove_dir_all(&path);
		fs::create_dir_all(&path).expect("create fixture");
		path
	}
}
