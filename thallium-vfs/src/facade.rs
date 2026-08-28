//! Capability-oriented asynchronous filesystem façade.
//!
//! This deliberately small interface follows `PHASE2-PLAN.md` section 2. Its
//! local/remote dispatch inspiration and the forwarding layers intentionally
//! omitted here are documented in `FS-VFS-RESEARCH.md` section 2.

use thallium_fs_core::{FileAttrs, FileEntry, FsResult, Location, NativePath};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{Capabilities, DirectoryReader, MoveOptions, MoveOutcome, StagedCopyOptions, StagedCopyOutcome};

/// Metadata lookup behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StatOptions {
	/// Follow a symbolic link and return target metadata when true.
	pub follow_symlinks: bool,
}

/// File-creation and replacement behavior for a writable handle.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpenWriteOptions {
	/// Create the file if it is absent.
	pub create:     bool,
	/// Require creation of a previously absent file.
	pub create_new: bool,
	/// Truncate an existing file before writing.
	pub truncate:   bool,
	/// Append writes to the existing content.
	pub append:     bool,
	/// Attributes to request when creating a file.
	pub attributes: Option<FileAttrs>,
}

/// Directory-creation behavior.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CreateDirOptions {
	/// Create missing parent directories as well.
	pub recursive:  bool,
	/// Attributes to apply to the created directory.
	pub attributes: Option<FileAttrs>,
}

/// Permanent-deletion behavior.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeleteOptions {
	/// Remove a directory tree rather than requiring an empty directory.
	pub recursive: bool,
}

/// Async filesystem operations implemented by each registered backend.
///
/// Associated handles keep runtime-specific resources inside this crate. The
/// trait itself contains no Qt, browser-state, or scheduler types.
pub trait FileSystem: Send + Sync {
	/// Backend-specific streaming directory reader.
	type Directory: DirectoryReader;
	/// Backend-specific readable file handle.
	type Reader: AsyncRead + Send + Unpin;
	/// Backend-specific writable file handle.
	type Writer: AsyncWrite + Send + Unpin;

	/// Describe the backend's supported operations and guarantees.
	fn capabilities(&self) -> Capabilities;

	/// Collect a complete directory snapshot.
	fn scan(&self, directory: &Location) -> impl Future<Output = FsResult<Vec<FileEntry>>> + Send;

	/// Open a streaming directory listing.
	fn read_dir(
		&self,
		directory: &Location,
	) -> impl Future<Output = FsResult<Self::Directory>> + Send;

	/// Read entry metadata at an explicit location.
	fn stat(
		&self,
		location: &Location,
		options: StatOptions,
	) -> impl Future<Output = FsResult<FileEntry>> + Send;

	/// Read the native target bytes stored by a symbolic link.
	fn read_link(&self, location: &Location) -> impl Future<Output = FsResult<NativePath>> + Send;

	/// Open a file for asynchronous reading.
	fn open_read(&self, location: &Location) -> impl Future<Output = FsResult<Self::Reader>> + Send;

	/// Open a file for asynchronous writing.
	fn open_write(
		&self,
		location: &Location,
		options: OpenWriteOptions,
	) -> impl Future<Output = FsResult<Self::Writer>> + Send;

	/// Create a directory at an explicit location.
	fn create_dir(
		&self,
		location: &Location,
		options: CreateDirOptions,
	) -> impl Future<Output = FsResult<()>> + Send;

	/// Reserve a task-owned temporary sibling for staged publication.
	fn create_temp(&self, destination: &Location) -> impl Future<Output = FsResult<Location>> + Send;

	/// Apply portable attributes to an entry.
	fn set_attrs(
		&self,
		location: &Location,
		attributes: &FileAttrs,
	) -> impl Future<Output = FsResult<()>> + Send;

	/// Create a symbolic link with a native, possibly non-UTF-8 target.
	fn symlink(
		&self,
		target: &NativePath,
		link: &Location,
	) -> impl Future<Output = FsResult<()>> + Send;

	/// Create a hard link to an entry on the same filesystem.
	fn hard_link(
		&self,
		source: &Location,
		link: &Location,
	) -> impl Future<Output = FsResult<()>> + Send;

	/// Rename an entry within a backend/filesystem.
	fn rename(
		&self,
		source: &Location,
		destination: &Location,
	) -> impl Future<Output = FsResult<()>> + Send;

	/// Move an entry, falling back to staged copy and source deletion on `EXDEV`.
	fn move_entry(
		&self,
		source: &Location,
		destination: &Location,
		options: MoveOptions,
	) -> impl Future<Output = FsResult<MoveOutcome>> + Send;

	/// Move an entry into the platform trash.
	fn trash(&self, location: &Location) -> impl Future<Output = FsResult<()>> + Send;

	/// Permanently delete an entry.
	fn delete(
		&self,
		location: &Location,
		options: DeleteOptions,
	) -> impl Future<Output = FsResult<()>> + Send;

	/// Copy through a task-owned staging path and publish the result.
	fn staged_copy(
		&self,
		source: &Location,
		destination: &Location,
		options: StagedCopyOptions,
	) -> impl Future<Output = FsResult<StagedCopyOutcome>> + Send;
}
