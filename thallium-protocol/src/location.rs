//! Owned filesystem rows and locations prepared for the cxx-qt adapter.
//!
//! The byte/display split follows `FS-VFS-RESEARCH.md` section 1. Raw Linux
//! path bytes remain authoritative; display strings are explicitly lossy and
//! are never sent back to the filesystem as identity.

use thallium_fs_core::{FileEntry, FileKind, FsError, FsErrorCode, Location, NativePath, Timestamp};

use crate::EntryIdDto;

/// Exact Linux path bytes plus their presentation-only rendering.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PathBytes {
	pub bytes:         Vec<u8>,
	pub display_lossy: String,
}

impl PathBytes {
	/// Creates a byte-preserving path value and derives its lossy display text.
	#[must_use]
	pub fn new(bytes: Vec<u8>) -> Self {
		let display_lossy = String::from_utf8_lossy(&bytes).into_owned();
		Self { bytes, display_lossy }
	}

	/// Reconstructs the native path used by the filesystem layer.
	#[must_use]
	pub fn to_native_path(&self) -> NativePath { NativePath::new(self.bytes.clone()) }
}

impl From<&NativePath> for PathBytes {
	fn from(path: &NativePath) -> Self { Self::new(path.as_bytes().to_vec()) }
}

impl From<NativePath> for PathBytes {
	fn from(path: NativePath) -> Self { Self::new(path.0) }
}

/// Backend-qualified location with no borrowed or runtime-owned data.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LocationDto {
	pub backend_id: u32,
	pub path:       PathBytes,
}

impl LocationDto {
	/// Converts back to the domain value when handling a frontend command.
	#[must_use]
	pub fn to_location(&self) -> Location {
		Location::new(thallium_fs_core::BackendId(self.backend_id), self.path.to_native_path())
	}
}

impl From<&Location> for LocationDto {
	fn from(location: &Location) -> Self {
		Self { backend_id: location.backend.0, path: PathBytes::from(&location.path) }
	}
}

impl From<Location> for LocationDto {
	fn from(location: Location) -> Self { Self::from(&location) }
}

/// Frontend-facing entry kind, kept independent of the filesystem enum's ABI.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum EntryKind {
	File,
	Directory,
	Symlink,
	BlockDevice,
	CharacterDevice,
	Fifo,
	Socket,
	#[default]
	Unknown,
}

impl From<FileKind> for EntryKind {
	fn from(kind: FileKind) -> Self {
		match kind {
			FileKind::File => Self::File,
			FileKind::Directory => Self::Directory,
			FileKind::Symlink => Self::Symlink,
			FileKind::BlockDevice => Self::BlockDevice,
			FileKind::CharacterDevice => Self::CharacterDevice,
			FileKind::Fifo => Self::Fifo,
			FileKind::Socket => Self::Socket,
			FileKind::Unknown => Self::Unknown,
		}
	}
}

/// Split timestamp that maps directly to primitive bridge fields.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimestampDto {
	pub seconds:     i64,
	pub nanoseconds: u32,
}

impl From<Timestamp> for TimestampDto {
	fn from(timestamp: Timestamp) -> Self {
		Self { seconds: timestamp.seconds, nanoseconds: timestamp.nanoseconds }
	}
}

/// One flat row for the current-directory list model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryEntryRow {
	pub id:             EntryIdDto,
	pub location:       LocationDto,
	pub name:           PathBytes,
	pub kind:           EntryKind,
	pub size:           u64,
	pub permissions:    u32,
	pub modified:       Option<TimestampDto>,
	pub is_hidden:      bool,
	pub symlink_target: Option<PathBytes>,
}

impl DirectoryEntryRow {
	/// Returns whether activating this row should navigate into it.
	#[must_use]
	pub const fn is_directory(&self) -> bool { matches!(self.kind, EntryKind::Directory) }
}

impl From<&FileEntry> for DirectoryEntryRow {
	fn from(entry: &FileEntry) -> Self {
		let name = PathBytes::from(&entry.name);
		Self {
			id: EntryIdDto::from(entry.id),
			location: LocationDto::from(&entry.location),
			is_hidden: name.bytes.first() == Some(&b'.'),
			name,
			kind: EntryKind::from(entry.metadata.kind),
			size: entry.metadata.size,
			permissions: entry.metadata.permissions,
			modified: entry.metadata.modified.map(TimestampDto::from),
			symlink_target: entry.symlink.as_ref().map(|link| PathBytes::from(&link.target)),
		}
	}
}

impl From<FileEntry> for DirectoryEntryRow {
	fn from(entry: FileEntry) -> Self { Self::from(&entry) }
}

/// Backwards-compatible name used by the state layer for a directory row.
pub type EntryDto = DirectoryEntryRow;

/// Stable, owned filesystem error prepared for frontend presentation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsErrorDto {
	pub code:     FsErrorCode,
	pub message:  String,
	pub os_code:  Option<i32>,
	pub location: Option<LocationDto>,
}

impl From<&FsError> for FsErrorDto {
	fn from(error: &FsError) -> Self {
		Self {
			code:     error.code,
			message:  error.message.clone(),
			os_code:  error.os_code,
			location: error.location.as_ref().map(LocationDto::from),
		}
	}
}

impl From<FsError> for FsErrorDto {
	fn from(error: FsError) -> Self { Self::from(&error) }
}

/// One complete generation of the current directory model.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DirectorySnapshot {
	pub location:   LocationDto,
	pub generation: u64,
	pub entries:    Vec<DirectoryEntryRow>,
	pub complete:   bool,
	pub error:      Option<FsErrorDto>,
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::{BackendId, EntryMetadata, FileEntry, FileKind, Location, NativePath, SymlinkInfo, Timestamp};

	use super::{DirectoryEntryRow, EntryKind, LocationDto, PathBytes};

	fn metadata(kind: FileKind) -> EntryMetadata {
		EntryMetadata {
			kind,
			size: 42,
			permissions: 0o100640,
			uid: 1000,
			gid: 1000,
			accessed: None,
			created: None,
			changed: None,
			modified: Some(Timestamp::new(12, 34)),
			device: 8,
			inode: 9,
			link_count: 1,
		}
	}

	#[test]
	fn path_conversion_preserves_non_utf8_identity() {
		let native = NativePath::from(&b"/tmp/\xff-file"[..]);
		let dto = PathBytes::from(&native);

		assert_eq!(dto.bytes, b"/tmp/\xff-file");
		assert_eq!(dto.display_lossy, "/tmp/�-file");
		assert_eq!(dto.to_native_path(), native);
	}

	#[test]
	fn location_round_trip_preserves_backend_and_bytes() {
		let location = Location::new(BackendId(7), NativePath::from(&b"/mnt/\xfe"[..]));
		assert_eq!(LocationDto::from(&location).to_location(), location);
	}

	#[test]
	fn entry_conversion_flattens_model_roles_without_losing_bytes() {
		let location = Location::local(NativePath::from(&b"/tmp/.\xff"[..]));
		let entry = FileEntry::new(
			location,
			metadata(FileKind::Symlink),
			Some(SymlinkInfo {
				target:          NativePath::from(&b"../\xfe"[..]),
				target_metadata: Some(metadata(FileKind::Directory)),
			}),
		);
		let row = DirectoryEntryRow::from(&entry);

		assert_eq!(row.id.to_entry_id(), entry.id);
		assert_eq!(row.name.bytes, b".\xff");
		assert_eq!(row.name.display_lossy, ".�");
		assert_eq!(row.kind, EntryKind::Symlink);
		assert!(row.is_hidden);
		assert_eq!(row.modified.expect("modified").nanoseconds, 34);
		assert_eq!(row.symlink_target.expect("target").bytes, b"../\xfe");
	}
}
