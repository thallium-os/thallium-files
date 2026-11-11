//! The Files app's own theme choice: follow the Thallium palette, or a
//! custom set of colours and the user's saved presets. The QML side owns the
//! format (JSON); this module only keeps the file, under
//! `$XDG_CONFIG_HOME/thallium/files-theme.json`.

use std::{fs, path::{Path, PathBuf}};

/// The saved theme, or an empty string before anything was saved.
pub fn read() -> String { read_from(&file()) }

/// Replace the saved theme. Written beside the file and renamed over it, so
/// a crash mid-write never leaves half a theme.
pub fn write(text: &str) -> Result<(), String> { write_to(&file(), text) }

fn read_from(path: &Path) -> String { fs::read_to_string(path).unwrap_or_default() }

fn write_to(path: &Path, text: &str) -> Result<(), String> {
	let fail = |error: std::io::Error| format!("{}: {error}", path.display());
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).map_err(fail)?;
	}
	let staged = path.with_extension("json.part");
	fs::write(&staged, text).map_err(fail)?;
	fs::rename(&staged, path).map_err(fail)
}

fn file() -> PathBuf {
	std::env::var_os("XDG_CONFIG_HOME")
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"))
		.join("thallium/files-theme.json")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_saved_theme_reads_back_and_a_missing_one_is_empty() {
		let dir = std::env::temp_dir().join(format!("thallium-files-theme-{}", std::process::id()));
		let path = dir.join("nested/files-theme.json");
		assert_eq!(read_from(&path), "");
		write_to(&path, r#"{"custom":true}"#).unwrap();
		write_to(&path, r#"{"custom":false}"#).unwrap();
		assert_eq!(read_from(&path), r#"{"custom":false}"#);
		assert!(!path.with_extension("json.part").exists());
		fs::remove_dir_all(dir).unwrap();
	}
}
