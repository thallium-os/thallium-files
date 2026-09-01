//! Dispatch between the local filesystem and rclone remotes.
//!
//! `Location` has always carried a `BackendId`; this is the type that finally
//! reads it. The scheduler and coordinator stay generic over one `FileSystem`,
//! so routing happens here rather than by changing either of them.

use core::{pin::Pin, task::{Context, Poll}};
use std::io;

use thallium_fs_core::{BackendId, FileAttrs, FileEntry, FsError, FsErrorCode, FsResult, Location, NativePath};
use thallium_vfs::{Capabilities, CreateDirOptions, DeleteOptions, DirectoryReader, FileSystem, LocalBackend, MoveOptions, MoveOutcome, OpenWriteOptions, StagedCopyOptions, StagedCopyOutcome, StatOptions, local::LocalDirectoryReader};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::backend::{RcloneBackend, RcloneDirectory, RcloneReader, RcloneWriter};

/// The filesystem the application runs on: the local disk, plus whatever
/// remotes rclone is configured for.
#[derive(Clone)]
pub struct RoutedFs {
	local:  LocalBackend,
	remote: Option<RcloneBackend>,
}

impl RoutedFs {
	/// A router with no remotes. Behaves exactly like the local backend.
	#[must_use]
	pub const fn local_only() -> Self { Self { local: LocalBackend::new(), remote: None } }

	#[must_use]
	pub fn with_remotes(remote: RcloneBackend) -> Self {
		Self { local: LocalBackend::new(), remote: Some(remote) }
	}

	/// The remote backend, when remote services are available this session.
	#[must_use]
	pub fn remotes(&self) -> Option<&RcloneBackend> { self.remote.as_ref() }

	fn remote_for(&self, location: &Location) -> FsResult<&RcloneBackend> {
		self.remote.as_ref().ok_or_else(|| {
			FsError::new(
				FsErrorCode::Unsupported,
				"remote services are not available in this session",
				Some(location.clone()),
			)
		})
	}

	fn is_local(location: &Location) -> bool { location.backend == BackendId::LOCAL }

	/// Copy between two different backends by streaming through this process.
	///
	/// Neither side can do this itself, so it is the router's job: read from
	/// wherever the source lives, write to wherever the destination lives, and
	/// publish under the destination backend's own rename.
	async fn copy_across(
		&self,
		source: &Location,
		destination: &Location,
		options: &StagedCopyOptions,
	) -> FsResult<StagedCopyOutcome> {
		let entry = self.stat(source, StatOptions { follow_symlinks: true }).await?;
		let temporary = self.create_temp(destination).await?;
		let mut reader = self.open_read(source).await?;
		let mut writer = self
			.open_write(&temporary, OpenWriteOptions {
				create:     true,
				create_new: false,
				truncate:   true,
				append:     false,
				attributes: None,
			})
			.await?;

		let copied = tokio::io::copy(&mut reader, &mut writer).await;
		let bytes_copied = match copied {
			Ok(bytes) => bytes,
			Err(error) => {
				let _ = self.delete(&temporary, DeleteOptions { recursive: false }).await;
				return Err(FsError::from_io(error, Some(source.clone())));
			}
		};
		if let Err(error) = tokio::io::AsyncWriteExt::shutdown(&mut writer).await {
			let _ = self.delete(&temporary, DeleteOptions { recursive: false }).await;
			return Err(FsError::from_io(error, Some(temporary)));
		}
		drop(writer);

		if let Some(attributes) = options.attributes.clone() {
			// Best effort: the destination backend decides whether attributes
			// mean anything to it.
			let _ = self.set_attrs(&temporary, &attributes).await;
		}
		if let Err(error) = self.rename(&temporary, destination).await {
			let _ = self.delete(&temporary, DeleteOptions { recursive: false }).await;
			return Err(error);
		}
		let _ = entry;
		Ok(StagedCopyOutcome { destination: destination.clone(), bytes_copied, published: true })
	}
}

impl Default for RoutedFs {
	fn default() -> Self { Self::local_only() }
}

/// A listing from whichever backend produced it.
pub enum RoutedDirectory {
	Local(LocalDirectoryReader),
	Remote(RcloneDirectory),
}

impl DirectoryReader for RoutedDirectory {
	async fn next_entry(&mut self) -> Result<Option<FileEntry>, FsError> {
		match self {
			Self::Local(reader) => reader.next_entry().await,
			Self::Remote(reader) => reader.next_entry().await,
		}
	}
}

/// A readable handle from whichever backend opened it.
pub enum RoutedReader {
	Local(tokio::fs::File),
	Remote(RcloneReader),
}

impl AsyncRead for RoutedReader {
	fn poll_read(
		self: Pin<&mut Self>,
		context: &mut Context<'_>,
		buffer: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		match self.get_mut() {
			Self::Local(file) => Pin::new(file).poll_read(context, buffer),
			Self::Remote(reader) => Pin::new(reader).poll_read(context, buffer),
		}
	}
}

/// A writable handle from whichever backend opened it.
pub enum RoutedWriter {
	Local(tokio::fs::File),
	Remote(RcloneWriter),
}

impl AsyncWrite for RoutedWriter {
	fn poll_write(
		self: Pin<&mut Self>,
		context: &mut Context<'_>,
		buffer: &[u8],
	) -> Poll<io::Result<usize>> {
		match self.get_mut() {
			Self::Local(file) => Pin::new(file).poll_write(context, buffer),
			Self::Remote(writer) => Pin::new(writer).poll_write(context, buffer),
		}
	}

	fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
		match self.get_mut() {
			Self::Local(file) => Pin::new(file).poll_flush(context),
			Self::Remote(writer) => Pin::new(writer).poll_flush(context),
		}
	}

	fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
		match self.get_mut() {
			Self::Local(file) => Pin::new(file).poll_shutdown(context),
			Self::Remote(writer) => Pin::new(writer).poll_shutdown(context),
		}
	}
}

impl FileSystem for RoutedFs {
	type Directory = RoutedDirectory;
	type Reader = RoutedReader;
	type Writer = RoutedWriter;

	/// The local filesystem's guarantees.
	///
	/// Capabilities are asked for without a location, so they cannot describe
	/// two backends at once. Remote operations that the local filesystem allows
	/// but a remote does not — trash, links — report `Unsupported` when called.
	fn capabilities(&self) -> Capabilities { self.local.capabilities() }

	async fn scan(&self, directory: &Location) -> FsResult<Vec<FileEntry>> {
		if Self::is_local(directory) {
			return self.local.scan(directory).await;
		}
		self.remote_for(directory)?.scan(directory).await
	}

	async fn read_dir(&self, directory: &Location) -> FsResult<Self::Directory> {
		if Self::is_local(directory) {
			return self.local.read_dir(directory).await.map(RoutedDirectory::Local);
		}
		self.remote_for(directory)?.read_dir(directory).await.map(RoutedDirectory::Remote)
	}

	async fn stat(&self, location: &Location, options: StatOptions) -> FsResult<FileEntry> {
		if Self::is_local(location) {
			return self.local.stat(location, options).await;
		}
		self.remote_for(location)?.stat(location, options).await
	}

	async fn read_link(&self, location: &Location) -> FsResult<NativePath> {
		if Self::is_local(location) {
			return self.local.read_link(location).await;
		}
		self.remote_for(location)?.read_link(location).await
	}

	async fn open_read(&self, location: &Location) -> FsResult<Self::Reader> {
		if Self::is_local(location) {
			return self.local.open_read(location).await.map(RoutedReader::Local);
		}
		self.remote_for(location)?.open_read(location).await.map(RoutedReader::Remote)
	}

	async fn open_write(
		&self,
		location: &Location,
		options: OpenWriteOptions,
	) -> FsResult<Self::Writer> {
		if Self::is_local(location) {
			return self.local.open_write(location, options).await.map(RoutedWriter::Local);
		}
		self.remote_for(location)?.open_write(location, options).await.map(RoutedWriter::Remote)
	}

	async fn create_dir(&self, location: &Location, options: CreateDirOptions) -> FsResult<()> {
		if Self::is_local(location) {
			return self.local.create_dir(location, options).await;
		}
		self.remote_for(location)?.create_dir(location, options).await
	}

	async fn create_temp(&self, destination: &Location) -> FsResult<Location> {
		if Self::is_local(destination) {
			return self.local.create_temp(destination).await;
		}
		self.remote_for(destination)?.create_temp(destination).await
	}

	async fn set_attrs(&self, location: &Location, attributes: &FileAttrs) -> FsResult<()> {
		if Self::is_local(location) {
			return self.local.set_attrs(location, attributes).await;
		}
		self.remote_for(location)?.set_attrs(location, attributes).await
	}

	async fn symlink(&self, target: &NativePath, link: &Location) -> FsResult<()> {
		if Self::is_local(link) {
			return self.local.symlink(target, link).await;
		}
		self.remote_for(link)?.symlink(target, link).await
	}

	async fn hard_link(&self, source: &Location, link: &Location) -> FsResult<()> {
		if Self::is_local(link) && Self::is_local(source) {
			return self.local.hard_link(source, link).await;
		}
		Err(FsError::new(
			FsErrorCode::Unsupported,
			"a hard link cannot cross backends",
			Some(link.clone()),
		))
	}

	async fn rename(&self, source: &Location, destination: &Location) -> FsResult<()> {
		if source.backend != destination.backend {
			// Read by the scheduler as its signal to copy instead.
			return Err(FsError::new(
				FsErrorCode::CrossDevice,
				"a rename cannot cross backends",
				Some(destination.clone()),
			));
		}
		if Self::is_local(source) {
			return self.local.rename(source, destination).await;
		}
		self.remote_for(source)?.rename(source, destination).await
	}

	async fn move_entry(
		&self,
		source: &Location,
		destination: &Location,
		options: MoveOptions,
	) -> FsResult<MoveOutcome> {
		if source.backend == destination.backend {
			return if Self::is_local(source) {
				self.local.move_entry(source, destination, options).await
			} else {
				self.remote_for(source)?.move_entry(source, destination, options).await
			};
		}
		let outcome = self.copy_across(source, destination, &options.copy).await?;
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

	async fn trash(&self, location: &Location) -> FsResult<()> {
		if Self::is_local(location) {
			return self.local.trash(location).await;
		}
		self.remote_for(location)?.trash(location).await
	}

	async fn delete(&self, location: &Location, options: DeleteOptions) -> FsResult<()> {
		if Self::is_local(location) {
			return self.local.delete(location, options).await;
		}
		self.remote_for(location)?.delete(location, options).await
	}

	async fn staged_copy(
		&self,
		source: &Location,
		destination: &Location,
		options: StagedCopyOptions,
	) -> FsResult<StagedCopyOutcome> {
		if source.backend == destination.backend {
			return if Self::is_local(source) {
				self.local.staged_copy(source, destination, options).await
			} else {
				self.remote_for(source)?.staged_copy(source, destination, options).await
			};
		}
		self.copy_across(source, destination, &options).await
	}
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::{BackendId, FsErrorCode, Location, NativePath};
	use thallium_vfs::FileSystem;

	use super::RoutedFs;

	fn remote(path: &[u8]) -> Location { Location::new(BackendId(1), NativePath::new(path.to_vec())) }

	#[tokio::test]
	async fn a_router_without_remotes_still_serves_the_local_disk() {
		let directory = tempfile::tempdir().expect("temporary directory");
		tokio::fs::write(directory.path().join("a.txt"), b"hello").await.expect("writes");
		let router = RoutedFs::local_only();
		let location = Location::local(NativePath::from(directory.path()));
		let entries = router.scan(&location).await.expect("scans");
		assert_eq!(entries.len(), 1);
		assert_eq!(entries[0].name.as_bytes(), b"a.txt");
	}

	#[tokio::test]
	async fn remote_locations_are_refused_when_no_remote_exists() {
		let router = RoutedFs::local_only();
		let error = router.scan(&remote(b"/photos")).await.expect_err("has no remote");
		assert_eq!(error.code, FsErrorCode::Unsupported);
	}

	#[tokio::test]
	async fn renames_across_backends_report_cross_device() {
		let router = RoutedFs::local_only();
		let error = router
			.rename(&Location::local(NativePath::new(b"/tmp/a".to_vec())), &remote(b"/a"))
			.await
			.expect_err("cannot cross backends");
		// This exact code is what makes the scheduler fall back to copying.
		assert_eq!(error.code, FsErrorCode::CrossDevice);
	}
}
