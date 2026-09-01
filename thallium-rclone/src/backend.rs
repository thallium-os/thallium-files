//! A `FileSystem` implementation backed by rclone remotes.
//!
//! Every method here is one remote-control call. Where rclone can do the work
//! server-side it is asked to: a copy inside one remote never moves bytes
//! through this process. Reads stream out of the daemon's object server and
//! writes stream into its upload endpoint, so neither direction stages a whole
//! file on local disk.

use core::{pin::Pin, sync::atomic::{AtomicU64, Ordering}, task::{Context, Poll, ready}};
use std::{collections::HashMap, io, sync::{Arc, Mutex}, time::{Duration, Instant}};

use serde_json::{Value, json};
use thallium_fs_core::{ConflictPolicy, EntryMetadata, FileAttrs, FileEntry, FileKind, FsError, FsErrorCode, FsResult, Location, NativePath, Timestamp};
use thallium_vfs::{Capabilities, CreateDirOptions, DeleteOptions, DirectoryReader, FileSystem, MoveOptions, MoveOutcome, OpenWriteOptions, StagedCopyOptions, StagedCopyOutcome, StatOptions};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::{rc::{Rclone, remote_error}, registry::{Remote, Remotes, remote_path}, time};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Permissions reported for remote entries.
///
/// Remotes have no Linux mode bits. These are presentation values so the
/// browser can render a familiar column; [`RcloneBackend::set_attrs`] does not
/// pretend to write them back.
const DIRECTORY_MODE: u32 = 0o040_755;
const FILE_MODE: u32 = 0o100_644;

/// VFS implementation over one rclone daemon serving every configured remote.
/// Directory listings held in memory, each with the moment it was read.
type Listings = Arc<Mutex<HashMap<Location, (Instant, Vec<FileEntry>)>>>;

/// How long a directory listing is served from memory before rclone is asked
/// again. Long enough that stepping into a folder and back is free, short
/// enough that a change made elsewhere shows up on its own.
const LISTING_TTL: Duration = Duration::from_secs(20);

#[derive(Clone)]
pub struct RcloneBackend {
	rclone:   Arc<Rclone>,
	remotes:  Remotes,
	/// Directory listings already fetched, with the moment each was read.
	///
	/// Every listing is a network round trip, so revisiting a folder without
	/// this meant waiting for the network again to see what was on screen a
	/// moment ago.
	listings: Listings,
}

impl RcloneBackend {
	#[must_use]
	pub fn new(rclone: Arc<Rclone>, remotes: Remotes) -> Self {
		Self { rclone, remotes, listings: Arc::default() }
	}

	/// A cached listing for `directory`, if one was read recently enough.
	fn cached(&self, directory: &Location) -> Option<Vec<FileEntry>> {
		let listings = self.listings.lock().ok()?;
		let (read, entries) = listings.get(directory)?;
		(read.elapsed() < LISTING_TTL).then(|| entries.clone())
	}

	fn remember(&self, directory: &Location, entries: &[FileEntry]) {
		if let Ok(mut listings) = self.listings.lock() {
			listings.insert(directory.clone(), (Instant::now(), entries.to_vec()));
		}
	}

	/// Drop every cached listing.
	///
	/// Called after anything that writes. A write can move an entry between two
	/// directories, and a rename changes both ends, so forgetting everything is
	/// the only answer that cannot be wrong.
	fn forget_listings(&self) {
		if let Ok(mut listings) = self.listings.lock() {
			listings.clear();
		}
	}

	#[must_use]
	pub fn remotes(&self) -> &Remotes { &self.remotes }

	/// The remote and rclone-relative path a location addresses.
	fn target(&self, location: &Location) -> FsResult<(Remote, String)> {
		Ok((self.remotes.resolve(location)?, remote_path(location)))
	}

	async fn call(&self, method: &str, body: Value, location: &Location) -> FsResult<Value> {
		self.rclone.call(method, body).await.map_err(|error| crate::rc::at(error, location))
	}

	/// Read one entry's metadata, or `None` when it is absent.
	async fn item(&self, location: &Location) -> FsResult<Option<FileEntry>> {
		let (remote, path) = self.target(location)?;
		if path.is_empty() {
			// rclone has no object for the root of a remote; the browser still
			// needs to be told it is a directory it may enter.
			return Ok(Some(entry_from_parts(location.clone(), true, 0, None)));
		}
		let payload =
			self.call("operations/stat", json!({ "fs": remote.fs(), "remote": path }), location).await?;
		let Some(item) = payload.get("item").filter(|item| !item.is_null()) else {
			return Ok(None);
		};
		Ok(Some(entry_from_item(location.backend, item)?))
	}

	async fn require(&self, location: &Location) -> FsResult<FileEntry> {
		self.item(location).await?.ok_or_else(|| {
			FsError::new(FsErrorCode::NotFound, "no such entry on the remote", Some(location.clone()))
		})
	}

	async fn is_directory(&self, location: &Location) -> FsResult<bool> {
		Ok(self.require(location).await?.metadata.kind == FileKind::Directory)
	}

	async fn exists(&self, location: &Location) -> FsResult<bool> {
		Ok(self.item(location).await?.is_some())
	}

	/// Mirrors the local backend's conflict handling so a copy onto a remote
	/// behaves the way the same copy behaves locally.
	async fn conflict_destination(
		&self,
		destination: &Location,
		policy: ConflictPolicy,
	) -> FsResult<Option<Location>> {
		if !self.exists(destination).await? {
			return Ok(Some(destination.clone()));
		}
		match policy {
			ConflictPolicy::Overwrite => Ok(Some(destination.clone())),
			ConflictPolicy::Skip => Ok(None),
			ConflictPolicy::Ask | ConflictPolicy::Abort => Err(FsError::new(
				FsErrorCode::AlreadyExists,
				"destination already exists and requires conflict resolution",
				Some(destination.clone()),
			)),
			ConflictPolicy::Rename => {
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
					if !self.exists(&candidate).await? {
						return Ok(Some(candidate));
					}
				}
				Err(FsError::new(
					FsErrorCode::AlreadyExists,
					"could not find an unused destination name",
					Some(destination.clone()),
				))
			}
		}
	}

	fn unsupported(&self, what: &str, location: &Location) -> FsError {
		FsError::new(FsErrorCode::Unsupported, what.to_owned(), Some(location.clone()))
	}
}

/// A listing already retrieved from the remote.
///
/// rclone answers `operations/list` with the whole directory, so there is
/// nothing to stream; the reader hands entries back one at a time to satisfy
/// the trait without pretending the underlying call was incremental.
pub struct RcloneDirectory {
	entries: std::vec::IntoIter<FileEntry>,
}

impl DirectoryReader for RcloneDirectory {
	async fn next_entry(&mut self) -> Result<Option<FileEntry>, FsError> { Ok(self.entries.next()) }
}

/// A file being read out of the daemon's object server.
pub struct RcloneReader {
	inner: Pin<Box<dyn AsyncRead + Send>>,
}

impl AsyncRead for RcloneReader {
	fn poll_read(
		mut self: Pin<&mut Self>,
		context: &mut Context<'_>,
		buffer: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		self.inner.as_mut().poll_read(context, buffer)
	}
}

/// A file being uploaded to a remote.
///
/// Bytes written here stream into an upload already in flight; nothing is
/// staged on local disk. The upload is only known to have succeeded once
/// `shutdown` has completed, which is where its failure surfaces.
pub struct RcloneWriter {
	pipe:     Option<tokio::io::DuplexStream>,
	upload:   Option<tokio::task::JoinHandle<FsResult<()>>>,
	location: Location,
}

impl AsyncWrite for RcloneWriter {
	fn poll_write(
		mut self: Pin<&mut Self>,
		context: &mut Context<'_>,
		buffer: &[u8],
	) -> Poll<io::Result<usize>> {
		match self.pipe.as_mut() {
			Some(pipe) => Pin::new(pipe).poll_write(context, buffer),
			None => Poll::Ready(Err(io::Error::from(io::ErrorKind::BrokenPipe))),
		}
	}

	fn poll_flush(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
		match self.pipe.as_mut() {
			Some(pipe) => Pin::new(pipe).poll_flush(context),
			None => Poll::Ready(Ok(())),
		}
	}

	fn poll_shutdown(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
		// Close the pipe first: the upload body ends at end-of-stream, so the
		// request cannot complete while this end is still open.
		if let Some(pipe) = self.pipe.as_mut() {
			ready!(Pin::new(pipe).poll_shutdown(context))?;
			self.pipe = None;
		}
		let Some(upload) = self.upload.as_mut() else {
			return Poll::Ready(Ok(()));
		};
		let finished = ready!(Pin::new(upload).poll(context));
		self.upload = None;
		Poll::Ready(match finished {
			Ok(Ok(())) => Ok(()),
			Ok(Err(error)) => Err(io::Error::other(error.message)),
			Err(error) => Err(io::Error::other(format!("upload task failed: {error}"))),
		})
	}
}

impl Drop for RcloneWriter {
	fn drop(&mut self) {
		// An abandoned writer must not leave an upload running against the
		// user's account; a cancelled copy has to leave nothing behind.
		if let Some(upload) = self.upload.take() {
			upload.abort();
		}
		let _ = &self.location;
	}
}

impl FileSystem for RcloneBackend {
	type Directory = RcloneDirectory;
	type Reader = RcloneReader;
	type Writer = RcloneWriter;

	fn capabilities(&self) -> Capabilities {
		Capabilities {
			// rclone's move is copy-then-delete on most providers, so a rename
			// cannot be claimed as an atomic publication.
			atomic_rename:    false,
			trash:            false,
			symlink:          false,
			hard_link:        false,
			sparse_files:     false,
			progressive_copy: false,
		}
	}

	async fn scan(&self, directory: &Location) -> FsResult<Vec<FileEntry>> {
		if let Some(entries) = self.cached(directory) {
			return Ok(entries);
		}
		let (remote, path) = self.target(directory)?;
		let payload =
			self.call("operations/list", json!({ "fs": remote.fs(), "remote": path }), directory).await?;
		let items = payload.get("list").and_then(Value::as_array).ok_or_else(|| {
			FsError::new(FsErrorCode::Io, "rclone returned no listing", Some(directory.clone()))
		})?;
		let mut entries = Vec::with_capacity(items.len());
		for item in items {
			entries.push(entry_from_item(directory.backend, item)?);
		}
		entries.sort_unstable_by(|left, right| left.name.cmp(&right.name));
		self.remember(directory, &entries);
		Ok(entries)
	}

	async fn read_dir(&self, directory: &Location) -> FsResult<Self::Directory> {
		Ok(RcloneDirectory { entries: self.scan(directory).await?.into_iter() })
	}

	async fn stat(&self, location: &Location, _options: StatOptions) -> FsResult<FileEntry> {
		// Remotes have no symbolic links, so following is the only behaviour.
		self.require(location).await
	}

	async fn read_link(&self, location: &Location) -> FsResult<NativePath> {
		Err(self.unsupported("remotes do not have symbolic links", location))
	}

	async fn open_read(&self, location: &Location) -> FsResult<Self::Reader> {
		let (remote, path) = self.target(location)?;
		let (user, password) = self.rclone.credentials();
		let response = self
			.rclone
			.http()
			.get(format!("{}/[{}]/{path}", self.rclone.base(), remote.fs()))
			.basic_auth(user, Some(password))
			.send()
			.await
			.map_err(|error| FsError::new(FsErrorCode::Io, error.to_string(), Some(location.clone())))?;
		if !response.status().is_success() {
			return Err(crate::rc::at(
				remote_error(&format!("could not read the object: {}", response.status())),
				location,
			));
		}
		let stream = futures_util::TryStreamExt::map_err(response.bytes_stream(), io::Error::other);
		Ok(RcloneReader { inner: Box::pin(tokio_util::io::StreamReader::new(stream)) })
	}

	async fn open_write(
		&self,
		location: &Location,
		_options: OpenWriteOptions,
	) -> FsResult<Self::Writer> {
		self.forget_listings();
		// Creation flags carry no meaning here: an upload always publishes the
		// object it names, and rclone has no open-for-append.
		let (remote, path) = self.target(location)?;
		let name = path.rsplit('/').next().unwrap_or_default().to_owned();
		if name.is_empty() {
			return Err(FsError::new(
				FsErrorCode::InvalidInput,
				"an upload needs a file name",
				Some(location.clone()),
			));
		}
		let directory = path.strip_suffix(&name).unwrap_or_default().trim_end_matches('/').to_owned();

		let (pipe, body_source) = tokio::io::duplex(64 * 1024);
		let client = self.rclone.http().clone();
		let base = self.rclone.base().to_owned();
		let (user, password) = self.rclone.credentials();
		let (user, password) = (user.to_owned(), password.to_owned());
		let fs = remote.fs();
		let upload = tokio::spawn(async move {
			let stream = tokio_util::io::ReaderStream::new(body_source);
			let part =
				reqwest::multipart::Part::stream(reqwest::Body::wrap_stream(stream)).file_name(name);
			let form = reqwest::multipart::Form::new().part("file", part);
			let response = client
				.post(format!("{base}/operations/uploadfile"))
				.basic_auth(&user, Some(&password))
				.query(&[("fs", fs.as_str()), ("remote", directory.as_str())])
				.multipart(form)
				.send()
				.await
				.map_err(|error| FsError::new(FsErrorCode::Io, error.to_string(), None))?;
			let status = response.status();
			let payload: Value = response.json().await.unwrap_or_else(|_| json!({}));
			if status.is_success() {
				return Ok(());
			}
			Err(remote_error(
				payload.get("error").and_then(Value::as_str).unwrap_or("the upload was rejected"),
			))
		});

		Ok(RcloneWriter { pipe: Some(pipe), upload: Some(upload), location: location.clone() })
	}

	async fn create_dir(&self, location: &Location, _options: CreateDirOptions) -> FsResult<()> {
		self.forget_listings();
		// rclone's mkdir always creates missing parents, so the recursive flag
		// has nothing to switch on.
		let (remote, path) = self.target(location)?;
		self.call("operations/mkdir", json!({ "fs": remote.fs(), "remote": path }), location).await?;
		Ok(())
	}

	async fn create_temp(&self, destination: &Location) -> FsResult<Location> {
		// Nothing is reserved on the remote: an object exists only once it has
		// been uploaded, so a staging name is all this can honestly return.
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
		let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
		let mut bytes = b".thallium-".to_vec();
		bytes.extend_from_slice(name.as_bytes());
		bytes.extend_from_slice(format!("-{}-{sequence}.part", std::process::id()).as_bytes());
		Ok(parent.join(NativePath::new(bytes)))
	}

	async fn set_attrs(&self, location: &Location, attributes: &FileAttrs) -> FsResult<()> {
		// Remotes carry no mode or ownership. Reporting an error here would fail
		// every copy that preserves attributes, so this records nothing and says
		// so rather than claiming a write that did not happen.
		let _ = (location, attributes);
		Ok(())
	}

	async fn symlink(&self, target: &NativePath, link: &Location) -> FsResult<()> {
		let _ = target;
		Err(self.unsupported("remotes do not support symbolic links", link))
	}

	async fn hard_link(&self, source: &Location, link: &Location) -> FsResult<()> {
		let _ = source;
		Err(self.unsupported("remotes do not support hard links", link))
	}

	async fn rename(&self, source: &Location, destination: &Location) -> FsResult<()> {
		self.forget_listings();
		if source.backend != destination.backend {
			// The scheduler reads this as its signal to fall back to copying.
			return Err(FsError::new(
				FsErrorCode::CrossDevice,
				"a rename cannot cross backends",
				Some(destination.clone()),
			));
		}
		let (remote, source_path) = self.target(source)?;
		let (_, destination_path) = self.target(destination)?;
		if self.is_directory(source).await? {
			self
				.call(
					"sync/move",
					json!({
						"srcFs": format!("{}{source_path}", remote.fs()),
						"dstFs": format!("{}{destination_path}", remote.fs()),
						"createEmptySrcDirs": true,
					}),
					destination,
				)
				.await?;
			// `sync/move` leaves the now-empty source directory behind.
			let _ = self
				.call("operations/rmdir", json!({ "fs": remote.fs(), "remote": source_path }), source)
				.await;
			return Ok(());
		}
		self
			.call(
				"operations/movefile",
				json!({
					"srcFs": remote.fs(),
					"srcRemote": source_path,
					"dstFs": remote.fs(),
					"dstRemote": destination_path,
				}),
				destination,
			)
			.await?;
		Ok(())
	}

	async fn move_entry(
		&self,
		source: &Location,
		destination: &Location,
		options: MoveOptions,
	) -> FsResult<MoveOutcome> {
		let Some(destination) =
			self.conflict_destination(destination, options.copy.conflict_policy).await?
		else {
			return Ok(MoveOutcome {
				destination:  destination.clone(),
				bytes_copied: 0,
				copied:       false,
				published:    false,
			});
		};
		self.rename(source, &destination).await?;
		Ok(MoveOutcome { destination, bytes_copied: 0, copied: false, published: true })
	}

	async fn trash(&self, location: &Location) -> FsResult<()> {
		Err(self.unsupported("remotes have no desktop trash; delete permanently instead", location))
	}

	async fn delete(&self, location: &Location, options: DeleteOptions) -> FsResult<()> {
		self.forget_listings();
		let (remote, path) = self.target(location)?;
		if path.is_empty() {
			return Err(FsError::new(
				FsErrorCode::InvalidInput,
				"refusing to delete the root of a remote",
				Some(location.clone()),
			));
		}
		let method = if self.is_directory(location).await? {
			if options.recursive { "operations/purge" } else { "operations/rmdir" }
		} else {
			"operations/deletefile"
		};
		self.call(method, json!({ "fs": remote.fs(), "remote": path }), location).await?;
		Ok(())
	}

	async fn staged_copy(
		&self,
		source: &Location,
		destination: &Location,
		options: StagedCopyOptions,
	) -> FsResult<StagedCopyOutcome> {
		if source.backend != destination.backend {
			return Err(FsError::new(
				FsErrorCode::CrossDevice,
				"a staged copy cannot cross backends",
				Some(destination.clone()),
			));
		}
		let entry = self.require(source).await?;
		if entry.metadata.kind == FileKind::Directory {
			return Err(FsError::new(
				FsErrorCode::IsDirectory,
				"staged copy supports files, not directory trees",
				Some(source.clone()),
			));
		}
		let Some(destination) = self.conflict_destination(destination, options.conflict_policy).await?
		else {
			return Ok(StagedCopyOutcome {
				destination:  destination.clone(),
				bytes_copied: 0,
				published:    false,
			});
		};

		// Server-side: the bytes never leave the provider.
		let (remote, source_path) = self.target(source)?;
		let temporary = self.create_temp(&destination).await?;
		let (_, temporary_path) = self.target(&temporary)?;
		self
			.call(
				"operations/copyfile",
				json!({
					"srcFs": remote.fs(),
					"srcRemote": source_path,
					"dstFs": remote.fs(),
					"dstRemote": temporary_path,
				}),
				&destination,
			)
			.await?;
		if let Err(error) = self.rename(&temporary, &destination).await {
			let _ = self.delete(&temporary, DeleteOptions { recursive: false }).await;
			return Err(error);
		}
		Ok(StagedCopyOutcome { destination, bytes_copied: entry.metadata.size, published: true })
	}
}

/// Build an entry from one `operations/list` or `operations/stat` item.
fn entry_from_item(backend: thallium_fs_core::BackendId, item: &Value) -> FsResult<FileEntry> {
	let path = item.get("Path").and_then(Value::as_str).unwrap_or_default();
	let name = item.get("Name").and_then(Value::as_str).unwrap_or_default();
	let is_directory = item.get("IsDir").and_then(Value::as_bool).unwrap_or(false);
	let size = item.get("Size").and_then(Value::as_i64).unwrap_or(0).max(0) as u64;
	let modified = item.get("ModTime").and_then(Value::as_str).and_then(time::parse);
	let location = Location::new(
		backend,
		NativePath::new(format!("/{}", path.trim_start_matches('/')).into_bytes()),
	);
	let _ = name;
	Ok(entry_from_parts(location, is_directory, size, modified))
}

fn entry_from_parts(
	location: Location,
	is_directory: bool,
	size: u64,
	modified: Option<Timestamp>,
) -> FileEntry {
	let metadata = EntryMetadata {
		kind: if is_directory { FileKind::Directory } else { FileKind::File },
		size,
		permissions: if is_directory { DIRECTORY_MODE } else { FILE_MODE },
		uid: 0,
		gid: 0,
		accessed: None,
		created: None,
		changed: None,
		modified,
		// A remote has no device or inode. The path hash keeps identity stable
		// across refreshes and distinct between entries, which is what the
		// browser's selection and delta reconciliation actually rely on.
		device: u64::from(location.backend.0),
		inode: path_identity(location.path.as_bytes()),
		link_count: 1,
	};
	FileEntry::new(location, metadata, None)
}

/// FNV-1a over the path, standing in for an inode number.
fn path_identity(path: &[u8]) -> u64 {
	path.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
		(hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
	})
}

#[cfg(test)]
mod tests {
	use serde_json::json;
	use thallium_fs_core::{BackendId, FileKind};

	use super::{entry_from_item, path_identity};

	#[test]
	fn a_listing_item_becomes_an_absolute_entry() {
		let entry = entry_from_item(
			BackendId(1),
			&json!({
				"Path": "photos/holiday.jpg",
				"Name": "holiday.jpg",
				"Size": 2048,
				"IsDir": false,
				"ModTime": "2026-05-20T11:17:44Z",
			}),
		)
		.expect("converts");
		assert_eq!(entry.location.backend, BackendId(1));
		assert_eq!(entry.location.path.as_bytes(), b"/photos/holiday.jpg");
		assert_eq!(entry.name.as_bytes(), b"holiday.jpg");
		assert_eq!(entry.metadata.size, 2048);
		assert_eq!(entry.metadata.kind, FileKind::File);
		assert_eq!(entry.metadata.modified.map(|time| time.seconds), Some(1_779_275_864));
	}

	#[test]
	fn directories_are_recognised_and_sized_zero() {
		let entry =
			entry_from_item(BackendId(2), &json!({ "Path": "photos", "Name": "photos", "IsDir": true }))
				.expect("converts");
		assert_eq!(entry.metadata.kind, FileKind::Directory);
		assert_eq!(entry.metadata.size, 0);
		assert_eq!(entry.metadata.modified, None);
	}

	#[test]
	fn identity_is_stable_per_path_and_differs_between_paths() {
		assert_eq!(path_identity(b"/a/b"), path_identity(b"/a/b"));
		assert_ne!(path_identity(b"/a/b"), path_identity(b"/a/c"));
	}
}
