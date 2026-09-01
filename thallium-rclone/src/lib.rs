//! Remote filesystems for Thallium Files, by way of rclone.
//!
//! rclone already speaks to every service a file manager would want to reach —
//! Google Drive, OneDrive, Dropbox, S3, SFTP, WebDAV and the rest — and it
//! already owns the credentials for them. This crate runs one `rclone rcd`
//! daemon and implements the VFS boundary against its remote-control API, so
//! remote entries reach the browser through the same `FileSystem` contract the
//! local disk does.
//!
//! Nothing here is provider-specific. A remote the user configures with
//! `rclone config` appears the next time [`connect`] runs.

pub mod authorize;
pub mod backend;
pub mod config;
pub mod rc;
pub mod registry;
pub mod routed;
pub mod time;

use std::sync::Arc;

pub use crate::{backend::RcloneBackend, rc::Rclone, registry::{Remote, Remotes}, routed::RoutedFs};

/// Everything remote services need, whether or not any are configured yet.
pub struct Remoting {
	/// The filesystem the application runs on.
	pub router:  RoutedFs,
	/// The running daemon, absent only when rclone is not installed.
	pub daemon:  Option<Arc<Rclone>>,
	/// The shared remote registry, updated in place as remotes are added.
	pub remotes: Remotes,
}

/// Start rclone and read the user's configured remotes.
///
/// A session with rclone installed but nothing configured still gets a daemon
/// and a live registry: that is precisely the session in which the user is
/// about to configure their first remote, and it has to become reachable
/// without restarting the application.
pub async fn connect() -> Remoting {
	let Ok(rclone) = Rclone::start().await else {
		return Remoting {
			router:  RoutedFs::local_only(),
			daemon:  None,
			remotes: Remotes::default(),
		};
	};
	let rclone = Arc::new(rclone);
	let remotes = Remotes::new(rclone.remotes().await.unwrap_or_default());
	let backend = RcloneBackend::new(Arc::clone(&rclone), remotes.clone());
	Remoting { router: RoutedFs::with_remotes(backend), daemon: Some(rclone), remotes }
}
