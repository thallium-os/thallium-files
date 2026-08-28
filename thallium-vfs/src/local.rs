//! Local Linux filesystem backend.
//!
//! All paths are explicit absolute [`Location`] values. File copies are written
//! to a task-owned sibling and renamed into place only after the copy and
//! requested attributes succeed, so a failed copy cannot expose a partial final
//! file.

use std::{io, os::unix::fs::{PermissionsExt, chown}, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}};

use thallium_fs_core::{BackendId, ConflictPolicy, EntryMetadata, FileAttrs, FileEntry, FileKind, FsError, FsErrorCode, FsResult, Location, NativePath, SymlinkInfo};

use crate::{Capabilities, CreateDirOptions, DeleteOptions, DirectoryReader, FileSystem, MoveOptions, MoveOutcome, OpenWriteOptions, StagedCopyOptions, StagedCopyOutcome, StatOptions};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// VFS implementation for the local Linux filesystem.
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalBackend;

impl LocalBackend {
	/// Construct a local backend without process-global filesystem state.
	#[must_use]
	pub const fn new() -> Self { Self }

	fn validate(location: &Location) -> FsResult<Location> {
		if location.backend != BackendId::LOCAL {
			return Err(FsError::new(
				FsErrorCode::Unsupported,
				format!("backend {} is not registered", location.backend.0),
				Some(location.clone()),
			));
		}
		if !location.is_absolute() {
			return Err(FsError::new(
				FsErrorCode::InvalidInput,
				"local filesystem locations must be absolute",
				Some(location.clone()),
			));
		}
		Ok(location.normalize())
	}

	fn path(location: &Location) -> FsResult<(Location, PathBuf)> {
		let normalized = Self::validate(location)?;
		let path = PathBuf::from(normalized.path.clone());
		Ok((normalized, path))
	}

	fn io(error: io::Error, location: &Location) -> FsError {
		FsError::from_io(error, Some(location.clone()))
	}

	async fn entry(location: Location, follow_symlinks: bool) -> FsResult<FileEntry> {
		let (_, path) = Self::path(&location)?;
		let link_metadata =
			tokio::fs::symlink_metadata(&path).await.map_err(|error| Self::io(error, &location))?;
		let is_symlink = link_metadata.file_type().is_symlink();
		let metadata = if follow_symlinks && is_symlink {
			tokio::fs::metadata(&path).await.map_err(|error| Self::io(error, &location))?
		} else {
			link_metadata.clone()
		};
		let symlink = if is_symlink {
			let target = tokio::fs::read_link(&path)
				.await
				.map(NativePath::from)
				.map_err(|error| Self::io(error, &location))?;
			let target_metadata =
				tokio::fs::metadata(&path).await.ok().map(|m| EntryMetadata::from_std(&m));
			Some(SymlinkInfo { target, target_metadata })
		} else {
			None
		};
		Ok(FileEntry::new(location, EntryMetadata::from_std(&metadata), symlink))
	}

	async fn exists(location: &Location) -> FsResult<bool> {
		let (_, path) = Self::path(location)?;
		tokio::fs::try_exists(path).await.map_err(|error| Self::io(error, location))
	}

	async fn conflict_destination(
		destination: &Location,
		policy: ConflictPolicy,
	) -> FsResult<Option<Location>> {
		if !Self::exists(destination).await? {
			return Ok(Some(Self::validate(destination)?));
		}

		match policy {
			ConflictPolicy::Overwrite => Ok(Some(Self::validate(destination)?)),
			ConflictPolicy::Skip => Ok(None),
			ConflictPolicy::Ask | ConflictPolicy::Abort => Err(FsError::new(
				FsErrorCode::AlreadyExists,
				"destination already exists and requires conflict resolution",
				Some(destination.clone()),
			)),
			ConflictPolicy::Rename => {
				let destination = Self::validate(destination)?;
				let parent = destination.parent().ok_or_else(|| {
					FsError::new(
						FsErrorCode::InvalidInput,
						"destination has no parent directory",
						Some(destination.clone()),
					)
				})?;
				let name = destination.path.file_name().ok_or_else(|| {
					FsError::new(
						FsErrorCode::InvalidInput,
						"destination has no file name",
						Some(destination.clone()),
					)
				})?;
				for suffix in 1..=10_000_u32 {
					let mut bytes = name.as_bytes().to_vec();
					bytes.extend_from_slice(format!(".{suffix}").as_bytes());
					let candidate = parent.join(NativePath::new(bytes));
					if !Self::exists(&candidate).await? {
						return Ok(Some(candidate));
					}
				}
				Err(FsError::new(
					FsErrorCode::AlreadyExists,
					"could not find an unused destination name",
					Some(destination),
				))
			}
		}
	}

	async fn remove_staging(location: &Location) {
		if let Ok((_, path)) = Self::path(location) {
			let _ = tokio::fs::remove_file(path).await;
		}
	}

	fn destructive_path(location: &Location) -> FsResult<(Location, PathBuf)> {
		let (location, path) = Self::path(location)?;
		if path == Path::new("/") {
			return Err(FsError::new(
				FsErrorCode::InvalidInput,
				"refusing to mutate the filesystem root",
				Some(location),
			));
		}
		Ok((location, path))
	}
}

/// Streaming local directory reader.
#[derive(Debug)]
pub struct LocalDirectoryReader {
	inner: tokio::fs::ReadDir,
}

impl DirectoryReader for LocalDirectoryReader {
	async fn next_entry(&mut self) -> Result<Option<FileEntry>, FsError> {
		let Some(entry) = self.inner.next_entry().await.map_err(FsError::from)? else {
			return Ok(None);
		};
		let location = Location::local(NativePath::from(entry.path()));
		LocalBackend::entry(location, false).await.map(Some)
	}
}

impl FileSystem for LocalBackend {
	type Directory = LocalDirectoryReader;
	type Reader = tokio::fs::File;
	type Writer = tokio::fs::File;

	fn capabilities(&self) -> Capabilities {
		Capabilities {
			atomic_rename:    true,
			trash:            true,
			symlink:          true,
			hard_link:        true,
			sparse_files:     false,
			progressive_copy: false,
		}
	}

	async fn scan(&self, directory: &Location) -> FsResult<Vec<FileEntry>> {
		let mut reader = self.read_dir(directory).await?;
		let mut entries = Vec::new();
		while let Some(entry) = reader.next_entry().await? {
			entries.push(entry);
		}
		entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));
		Ok(entries)
	}

	async fn read_dir(&self, directory: &Location) -> FsResult<Self::Directory> {
		let (directory, path) = Self::path(directory)?;
		let inner = tokio::fs::read_dir(path).await.map_err(|error| Self::io(error, &directory))?;
		Ok(LocalDirectoryReader { inner })
	}

	async fn stat(&self, location: &Location, options: StatOptions) -> FsResult<FileEntry> {
		Self::entry(Self::validate(location)?, options.follow_symlinks).await
	}

	async fn read_link(&self, location: &Location) -> FsResult<NativePath> {
		let (location, path) = Self::path(location)?;
		tokio::fs::read_link(path)
			.await
			.map(NativePath::from)
			.map_err(|error| Self::io(error, &location))
	}

	async fn open_read(&self, location: &Location) -> FsResult<Self::Reader> {
		let (location, path) = Self::path(location)?;
		tokio::fs::File::open(path).await.map_err(|error| Self::io(error, &location))
	}

	async fn open_write(
		&self,
		location: &Location,
		options: OpenWriteOptions,
	) -> FsResult<Self::Writer> {
		let (location, path) = Self::path(location)?;
		let mut open = tokio::fs::OpenOptions::new();
		open
			.write(true)
			.create(options.create)
			.create_new(options.create_new)
			.truncate(options.truncate)
			.append(options.append);
		let file = open.open(path).await.map_err(|error| Self::io(error, &location))?;
		if let Some(attributes) = options.attributes {
			self.set_attrs(&location, &attributes).await?;
		}
		Ok(file)
	}

	async fn create_dir(&self, location: &Location, options: CreateDirOptions) -> FsResult<()> {
		let (location, path) = Self::path(location)?;
		let result = if options.recursive {
			tokio::fs::create_dir_all(path).await
		} else {
			tokio::fs::create_dir(path).await
		};
		result.map_err(|error| Self::io(error, &location))?;
		if let Some(attributes) = options.attributes {
			self.set_attrs(&location, &attributes).await?;
		}
		Ok(())
	}

	async fn create_temp(&self, destination: &Location) -> FsResult<Location> {
		let destination = Self::validate(destination)?;
		let parent = destination.parent().ok_or_else(|| {
			FsError::new(
				FsErrorCode::InvalidInput,
				"destination has no parent directory",
				Some(destination.clone()),
			)
		})?;
		let name = destination.path.file_name().ok_or_else(|| {
			FsError::new(
				FsErrorCode::InvalidInput,
				"destination has no file name",
				Some(destination.clone()),
			)
		})?;

		for _ in 0..100 {
			let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
			let mut bytes = b".thallium-".to_vec();
			bytes.extend_from_slice(name.as_bytes());
			bytes.extend_from_slice(format!("-{}-{sequence}.part", std::process::id()).as_bytes());
			let temporary = parent.join(NativePath::new(bytes));
			let (_, path) = Self::path(&temporary)?;
			match tokio::fs::OpenOptions::new().write(true).create_new(true).open(path).await {
				Ok(_) => return Ok(temporary),
				Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
				Err(error) => return Err(Self::io(error, &temporary)),
			}
		}
		Err(FsError::new(
			FsErrorCode::AlreadyExists,
			"could not reserve a staging file",
			Some(destination),
		))
	}

	async fn set_attrs(&self, location: &Location, attributes: &FileAttrs) -> FsResult<()> {
		let (location, path) = Self::path(location)?;
		if let Some(mode) = attributes.permissions {
			tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
				.await
				.map_err(|error| Self::io(error, &location))?;
		}

		if attributes.uid.is_some() || attributes.gid.is_some() {
			let ownership_path = path.clone();
			let uid = attributes.uid;
			let gid = attributes.gid;
			let result =
				tokio::task::spawn_blocking(move || chown(ownership_path, uid, gid)).await.map_err(
					|error| FsError::new(FsErrorCode::Io, error.to_string(), Some(location.clone())),
				)?;
			if let Err(error) = result {
				// Metadata preservation is best-effort when ownership changes require
				// privileges the application does not have.
				if error.kind() != io::ErrorKind::PermissionDenied {
					return Err(Self::io(error, &location));
				}
			}
		}

		if attributes.accessed.is_some() || attributes.modified.is_some() {
			let metadata =
				tokio::fs::metadata(&path).await.map_err(|error| Self::io(error, &location))?;
			let accessed = attributes.accessed.map_or_else(
				|| filetime::FileTime::from_last_access_time(&metadata),
				|time| filetime::FileTime::from_unix_time(time.seconds, time.nanoseconds),
			);
			let modified = attributes.modified.map_or_else(
				|| filetime::FileTime::from_last_modification_time(&metadata),
				|time| filetime::FileTime::from_unix_time(time.seconds, time.nanoseconds),
			);
			let time_path = path;
			let result = tokio::task::spawn_blocking(move || {
				filetime::set_file_times(time_path, accessed, modified)
			})
			.await
			.map_err(|error| FsError::new(FsErrorCode::Io, error.to_string(), Some(location.clone())))?;
			result.map_err(|error| Self::io(error, &location))?;
		}
		Ok(())
	}

	async fn symlink(&self, target: &NativePath, link: &Location) -> FsResult<()> {
		let (link, path) = Self::path(link)?;
		tokio::fs::symlink(target.as_path(), path).await.map_err(|error| Self::io(error, &link))
	}

	async fn hard_link(&self, source: &Location, link: &Location) -> FsResult<()> {
		let (source, source_path) = Self::path(source)?;
		let (link, link_path) = Self::path(link)?;
		let _ = source;
		tokio::fs::hard_link(source_path, link_path).await.map_err(|error| Self::io(error, &link))
	}

	async fn rename(&self, source: &Location, destination: &Location) -> FsResult<()> {
		let (source, source_path) = Self::destructive_path(source)?;
		let (destination, destination_path) = Self::destructive_path(destination)?;
		let _ = source;
		tokio::fs::rename(source_path, destination_path)
			.await
			.map_err(|error| Self::io(error, &destination))
	}

	async fn move_entry(
		&self,
		source: &Location,
		destination: &Location,
		options: MoveOptions,
	) -> FsResult<MoveOutcome> {
		let Some(destination) =
			Self::conflict_destination(destination, options.copy.conflict_policy).await?
		else {
			return Ok(MoveOutcome {
				destination:  destination.clone(),
				bytes_copied: 0,
				copied:       false,
				published:    false,
			});
		};
		match self.rename(source, &destination).await {
			Ok(()) => Ok(MoveOutcome { destination, bytes_copied: 0, copied: false, published: true }),
			Err(error) if error.code == FsErrorCode::CrossDevice => {
				let outcome = self.staged_copy(source, &destination, options.copy).await?;
				if outcome.published {
					self.delete(source, options.delete).await?;
				}
				Ok(MoveOutcome {
					destination:  outcome.destination,
					bytes_copied: outcome.bytes_copied,
					copied:       true,
					published:    outcome.published,
				})
			}
			Err(error) => Err(error),
		}
	}

	async fn trash(&self, location: &Location) -> FsResult<()> {
		let (location, path) = Self::destructive_path(location)?;
		let result =
			tokio::task::spawn_blocking(move || trash::delete(path).map_err(|error| error.to_string()))
				.await
				.map_err(|error| {
					FsError::new(FsErrorCode::Io, error.to_string(), Some(location.clone()))
				})?;
		result.map_err(|message| FsError::new(FsErrorCode::Io, message, Some(location)))
	}

	async fn delete(&self, location: &Location, options: DeleteOptions) -> FsResult<()> {
		let (location, path) = Self::destructive_path(location)?;
		let metadata =
			tokio::fs::symlink_metadata(&path).await.map_err(|error| Self::io(error, &location))?;
		let result = if metadata.file_type().is_dir() {
			if options.recursive {
				tokio::fs::remove_dir_all(path).await
			} else {
				tokio::fs::remove_dir(path).await
			}
		} else {
			tokio::fs::remove_file(path).await
		};
		result.map_err(|error| Self::io(error, &location))
	}

	async fn staged_copy(
		&self,
		source: &Location,
		destination: &Location,
		options: StagedCopyOptions,
	) -> FsResult<StagedCopyOutcome> {
		let source_entry = self.stat(source, StatOptions { follow_symlinks: true }).await?;
		if source_entry.metadata.kind == FileKind::Directory {
			return Err(FsError::new(
				FsErrorCode::IsDirectory,
				"MVP staged copy supports files, not directory trees",
				Some(source.clone()),
			));
		}
		let Some(destination) =
			Self::conflict_destination(destination, options.conflict_policy).await?
		else {
			return Ok(StagedCopyOutcome {
				destination:  destination_for_skip(destination),
				bytes_copied: 0,
				published:    false,
			});
		};
		let temporary = self.create_temp(&destination).await?;
		let (_, source_path) = Self::path(source)?;
		let (_, temporary_path) = Self::path(&temporary)?;
		let copy_result =
			tokio::fs::copy(source_path, temporary_path).await.map_err(|error| Self::io(error, source));
		let bytes_copied = match copy_result {
			Ok(bytes) => bytes,
			Err(error) => {
				Self::remove_staging(&temporary).await;
				return Err(error);
			}
		};
		let attributes =
			options.attributes.unwrap_or_else(|| FileAttrs::from_metadata(&source_entry.metadata));
		if let Err(error) = self.set_attrs(&temporary, &attributes).await {
			Self::remove_staging(&temporary).await;
			return Err(error);
		}
		if let Err(error) = self.rename(&temporary, &destination).await {
			Self::remove_staging(&temporary).await;
			return Err(error);
		}
		Ok(StagedCopyOutcome { destination, bytes_copied, published: true })
	}
}

fn destination_for_skip(destination: &Location) -> Location { destination.clone() }
