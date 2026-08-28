//! Backend-owned asynchronous handle traits.
//!
//! These traits keep concrete local handles behind the VFS façade, following
//! `FS-VFS-RESEARCH.md` section 2 without reproducing Yazi's forwarding enums.

use thallium_fs_core::{FileEntry, FsError};
use tokio::io::{AsyncRead, AsyncWrite};

/// An asynchronous directory listing owned by a backend.
pub trait DirectoryReader: Send {
	/// Return the next entry, or `None` when the listing is exhausted.
	fn next_entry(&mut self) -> impl Future<Output = Result<Option<FileEntry>, FsError>> + Send;
}

/// An asynchronous readable file handle.
pub trait ReadHandle: AsyncRead + Send + Unpin {}

impl<T> ReadHandle for T where T: AsyncRead + Send + Unpin {}

/// An asynchronous writable file handle.
pub trait WriteHandle: AsyncWrite + Send + Unpin {}

impl<T> WriteHandle for T where T: AsyncWrite + Send + Unpin {}
