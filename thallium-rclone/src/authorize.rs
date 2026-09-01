//! Browser consent for remotes that need OAuth.
//!
//! rclone's non-interactive configuration branches at `config_is_local`. Saying
//! yes runs the consent flow inside the `config/create` call, which does not
//! return until the browser comes back; saying no takes the headless branch,
//! which asks for a token and points at <https://rclone.org/remote_setup/> for
//! how to get one. A graphical client can use neither: the first freezes the
//! interface for as long as consent takes, and the second hands the user a
//! documentation page instead of a login.
//!
//! So this takes the headless branch and runs `rclone authorize` itself. The
//! wait lives in a child process, the link is the real consent URL, and the
//! token it prints is the answer rclone is already waiting for.

use std::{process::Stdio, sync::{Arc, Mutex}};

use tokio::{io::{AsyncBufReadExt, BufReader}, process::Command};

/// A consent flow as the frontend sees it. Every field is empty until the flow
/// reaches the point that fills it.
#[derive(Clone, Default)]
pub struct Consent {
	/// The link to open in a browser.
	pub url:   String,
	/// The token to hand back to rclone once consent has been granted.
	pub token: String,
	/// Why the flow ended without a token.
	pub error: String,
}

/// Read by the frontend on every poll; written by the child's readers.
pub type Shared = Arc<Mutex<Consent>>;

/// The notice rclone prints before it begins waiting.
const LINK_NOTICE: &str = "Please go to the following link: ";
/// The token is printed between these markers.
const PASTE_OPEN: &str = "--->";
const PASTE_CLOSE: &str = "<---";

/// Start `rclone authorize` for `kind`, reporting progress into the handle.
///
/// Must be called from within a Tokio runtime.
#[must_use]
pub fn start(kind: &str) -> Shared {
	let shared: Shared = Arc::new(Mutex::new(Consent::default()));
	let progress = Arc::clone(&shared);
	let kind = kind.to_owned();
	tokio::spawn(async move {
		if let Err(message) = run(&kind, &progress).await
			&& let Ok(mut consent) = progress.lock()
		{
			consent.error = message;
		}
	});
	shared
}

async fn run(kind: &str, progress: &Shared) -> Result<(), String> {
	let mut child = Command::new("rclone")
		.arg("authorize")
		.arg(kind)
		// rclone opens the browser itself otherwise, and a failure to do so is
		// silent. The frontend opens the link, so it can say when it cannot.
		.arg("--auth-no-open-browser")
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.kill_on_drop(true)
		.spawn()
		.map_err(|error| format!("rclone authorize could not start: {error}"))?;

	let stderr = child.stderr.take().ok_or("rclone authorize produced no notices")?;
	// The link arrives on stderr well before the child exits, so it is read as
	// it comes rather than waited for. The lines are kept because the token has
	// been seen on either stream depending on the build.
	let notices: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
	let watched = Arc::clone(progress);
	let collected = Arc::clone(&notices);
	let reader = tokio::spawn(async move {
		let mut lines = BufReader::new(stderr).lines();
		while let Ok(Some(line)) = lines.next_line().await {
			if let Some(at) = line.find(LINK_NOTICE)
				&& let Ok(mut consent) = watched.lock()
			{
				consent.url = line[at + LINK_NOTICE.len()..].trim().to_owned();
			}
			if let Ok(mut text) = collected.lock() {
				text.push_str(&line);
				text.push('\n');
			}
		}
	});

	let output =
		child.wait_with_output().await.map_err(|error| format!("rclone authorize failed: {error}"))?;
	let _ = reader.await;

	let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
	let stderr = notices.lock().map(|text| text.clone()).unwrap_or_default();
	let token = pasted(&stdout)
		.or_else(|| pasted(&stderr))
		.ok_or_else(|| "rclone authorize ended without granting a token".to_owned())?;
	if let Ok(mut consent) = progress.lock() {
		consent.token = token;
	}
	Ok(())
}

/// The token rclone prints between its paste markers.
fn pasted(text: &str) -> Option<String> {
	let start = text.find(PASTE_OPEN)? + PASTE_OPEN.len();
	let rest = &text[start..];
	let end = rest.find(PASTE_CLOSE)?;
	let token = rest[..end].trim();
	(!token.is_empty()).then(|| token.to_owned())
}

#[cfg(test)]
mod tests {
	use super::{LINK_NOTICE, pasted};

	#[test]
	fn the_token_is_taken_from_between_the_paste_markers() {
		let text = "Paste the following into your remote machine --->\n{\"access_token\":\"a\"}\n<---End \
		            paste\n";
		assert_eq!(pasted(text).as_deref(), Some("{\"access_token\":\"a\"}"));
	}

	#[test]
	fn output_without_markers_yields_no_token() {
		assert_eq!(pasted("Waiting for code...\n"), None);
		assert_eq!(pasted("--->\n\n<---"), None);
	}

	#[test]
	fn the_link_notice_is_the_prefix_rclone_prints() {
		let line = "2026/09/01 03:18:54 NOTICE: Please go to the following link: \
		            http://127.0.0.1:53682/auth?state=TT3HS";
		let at = line.find(LINK_NOTICE).expect("notice");
		assert_eq!(&line[at + LINK_NOTICE.len()..], "http://127.0.0.1:53682/auth?state=TT3HS");
	}
}
