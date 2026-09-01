//! The `rclone rcd` process and its JSON-RPC client.
//!
//! Thallium Files does not link rclone; it runs one `rcd` daemon on loopback
//! and speaks the documented remote-control API to it. That keeps every
//! provider rclone supports available without a provider-specific line of code
//! here, and it keeps rclone's own credential storage the only place tokens
//! live.

use std::{io, net::TcpListener, process::Stdio, time::{Duration, Instant}};

use serde_json::{Value, json};
use thallium_fs_core::{FsError, FsErrorCode, FsResult, Location};
use tokio::process::{Child, Command};

/// How long to wait for a freshly spawned daemon to answer.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
/// How long any single call may take. Cloud round-trips are slow, but a call
/// that has not answered by now is a hang, not latency.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// A running `rclone rcd` daemon and the client that talks to it.
pub struct Rclone {
	client: reqwest::Client,
	base:   String,
	user:   String,
	pass:   String,
	/// Kept so the daemon dies with this process rather than outliving it.
	child:  Child,
}

impl Rclone {
	/// Spawn `rclone rcd` on a loopback port and wait for it to answer.
	///
	/// `--rc-serve` is required: it is what makes object bodies readable over
	/// HTTP, which is how [`crate::backend::RcloneBackend`] streams file reads.
	pub async fn start() -> FsResult<Self> {
		let port = free_port()?;
		let user = "thallium".to_owned();
		let pass = secret()?;
		let base = format!("http://127.0.0.1:{port}");

		let mut command = Command::new("rclone");
		// Die with the application. `kill_on_drop` only fires on an orderly
		// exit; a signalled or crashed process would otherwise leave a daemon
		// running with the user's cloud credentials live in it.
		unsafe {
			command.pre_exec(|| {
				if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
					return Err(io::Error::last_os_error());
				}
				// The parent may already be gone, in which case the signal was
				// delivered before this was set and will never arrive.
				if libc::getppid() == 1 {
					return Err(io::Error::from(io::ErrorKind::BrokenPipe));
				}
				Ok(())
			});
		}
		let child = command
			.arg("rcd")
			.arg("--rc-addr")
			.arg(format!("127.0.0.1:{port}"))
			.arg("--rc-user")
			.arg(&user)
			.arg("--rc-pass")
			.arg(&pass)
			.arg("--rc-serve")
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.kill_on_drop(true)
			.spawn()
			.map_err(|error| match error.kind() {
				io::ErrorKind::NotFound => FsError::new(
					FsErrorCode::Unsupported,
					"rclone is not installed, so remote services are unavailable",
					None,
				),
				_ => FsError::new(FsErrorCode::Io, format!("could not start rclone rcd: {error}"), None),
			})?;

		let rclone = Self { client: reqwest::Client::new(), base, user, pass, child };
		rclone.await_ready().await?;
		Ok(rclone)
	}

	/// The base URL of the daemon, for callers that read object bodies directly.
	pub fn base(&self) -> &str { &self.base }

	/// Credentials for those same direct reads.
	pub fn credentials(&self) -> (&str, &str) { (&self.user, &self.pass) }

	pub fn http(&self) -> &reqwest::Client { &self.client }

	/// Call one remote-control method.
	pub async fn call(&self, method: &str, body: Value) -> FsResult<Value> {
		let response = self
			.client
			.post(format!("{}/{method}", self.base))
			.basic_auth(&self.user, Some(&self.pass))
			.timeout(CALL_TIMEOUT)
			.json(&body)
			.send()
			.await
			.map_err(|error| transport_error(method, &error))?;

		let status = response.status();
		let payload: Value = response.json().await.unwrap_or_else(|_| json!({}));
		if status.is_success() {
			return Ok(payload);
		}
		Err(remote_error(payload.get("error").and_then(Value::as_str).unwrap_or("rclone call failed")))
	}

	/// Remote names configured in the user's own rclone configuration.
	pub async fn remotes(&self) -> FsResult<Vec<String>> {
		let payload = self.call("config/listremotes", json!({})).await?;
		Ok(
			payload
				.get("remotes")
				.and_then(Value::as_array)
				.map(|remotes| {
					remotes.iter().filter_map(Value::as_str).map(str::to_owned).collect::<Vec<_>>()
				})
				.unwrap_or_default(),
		)
	}

	async fn await_ready(&self) -> FsResult<()> {
		let deadline = Instant::now() + STARTUP_TIMEOUT;
		loop {
			if self.call("core/version", json!({})).await.is_ok() {
				return Ok(());
			}
			if Instant::now() >= deadline {
				return Err(FsError::new(FsErrorCode::Io, "rclone rcd did not become ready", None));
			}
			tokio::time::sleep(Duration::from_millis(100)).await;
		}
	}
}

impl Drop for Rclone {
	fn drop(&mut self) {
		// `kill_on_drop` covers the child; this makes the intent explicit for a
		// daemon that would otherwise keep the user's tokens in memory.
		let _ = self.child.start_kill();
	}
}

/// Map an rclone error string onto the filesystem error vocabulary.
///
/// rclone reports failures as prose, so this reads it. An unrecognised message
/// stays `Io` and keeps its original text rather than being flattened.
pub fn remote_error(message: &str) -> FsError {
	let lower = message.to_ascii_lowercase();
	let code = if lower.contains("not found") || lower.contains("no such") {
		FsErrorCode::NotFound
	} else if lower.contains("already exists") {
		FsErrorCode::AlreadyExists
	} else if lower.contains("permission") || lower.contains("unauthor") || lower.contains("403") {
		FsErrorCode::PermissionDenied
	} else if lower.contains("directory not empty") {
		FsErrorCode::DirectoryNotEmpty
	} else if lower.contains("is a directory") {
		FsErrorCode::IsDirectory
	} else if lower.contains("not a directory") {
		FsErrorCode::NotDirectory
	} else if lower.contains("quota") || lower.contains("no space") {
		FsErrorCode::NoSpace
	} else if lower.contains("read only") || lower.contains("read-only") {
		FsErrorCode::ReadOnly
	} else {
		FsErrorCode::Io
	};
	FsError::new(code, message.to_owned(), None)
}

/// Attach a location to an error raised before one was known.
pub fn at(error: FsError, location: &Location) -> FsError {
	FsError::new(error.code, error.message, Some(location.clone()))
}

fn transport_error(method: &str, error: &reqwest::Error) -> FsError {
	let code = if error.is_timeout() { FsErrorCode::Busy } else { FsErrorCode::Io };
	FsError::new(code, format!("rclone {method}: {error}"), None)
}

/// Bind port zero and give the port straight back. There is a window before
/// rclone claims it, which is why startup is confirmed rather than assumed.
fn free_port() -> FsResult<u16> {
	let listener = TcpListener::bind("127.0.0.1:0")
		.map_err(|error| FsError::new(FsErrorCode::Io, error.to_string(), None))?;
	listener
		.local_addr()
		.map(|address| address.port())
		.map_err(|error| FsError::new(FsErrorCode::Io, error.to_string(), None))
}

/// A per-run password for the loopback socket. Without one, every local process
/// could drive the user's cloud accounts through this daemon.
fn secret() -> FsResult<String> {
	// Read exactly what is needed: `/dev/urandom` never reaches end-of-file, so
	// reading it whole never returns.
	use std::io::Read;
	let mut bytes = [0_u8; 32];
	std::fs::File::open("/dev/urandom")
		.and_then(|mut source| source.read_exact(&mut bytes))
		.map_err(|error| FsError::new(FsErrorCode::Io, error.to_string(), None))?;
	Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::FsErrorCode;

	use super::{free_port, remote_error, secret};

	#[test]
	fn error_prose_maps_onto_codes() {
		assert_eq!(remote_error("directory not found").code, FsErrorCode::NotFound);
		assert_eq!(
			remote_error("403 Forbidden: permission denied").code,
			FsErrorCode::PermissionDenied
		);
		assert_eq!(remote_error("directory not empty").code, FsErrorCode::DirectoryNotEmpty);
		assert_eq!(remote_error("quota exceeded").code, FsErrorCode::NoSpace);
		assert_eq!(remote_error("something rclone has never said").code, FsErrorCode::Io);
	}

	#[test]
	fn the_original_message_survives_mapping() {
		assert_eq!(remote_error("object not found").message, "object not found");
	}

	#[test]
	fn a_secret_is_long_and_never_repeats() {
		let first = secret().expect("urandom is readable");
		assert_eq!(first.len(), 64);
		assert_ne!(first, secret().expect("urandom is readable"));
	}

	#[test]
	fn a_free_port_is_a_real_port() {
		assert!(free_port().expect("loopback binds") > 0);
	}
}
