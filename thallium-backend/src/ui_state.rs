//! Small interface preferences that outlive the window: which sidebar tab
//! is open, whether it is collapsed, and recent searches.
//!
//! They live as `key = value` lines in `$XDG_CONFIG_HOME/thallium/files.conf`
//! (or `~/.config/thallium/files.conf`), next to the shared Thallium theme
//! file, so they are plain text a person can read and edit.

use std::{collections::BTreeMap, fmt::Write as _, path::{Path, PathBuf}};

/// The preference file, or `None` without a home folder.
#[must_use]
pub fn path() -> Option<PathBuf> {
	let base = std::env::var_os("XDG_CONFIG_HOME")
		.filter(|value| !value.is_empty())
		.map(PathBuf::from)
		.or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
	Some(base.join("thallium").join("files.conf"))
}

/// Read every preference in `file`; a missing or unreadable file is empty.
#[must_use]
pub fn load(file: &Path) -> BTreeMap<String, String> {
	std::fs::read_to_string(file).map(|text| parse(&text)).unwrap_or_default()
}

/// Write `values` to `file`, creating its folder, through a temporary file so
/// a crash never leaves it half written.
pub fn save(file: &Path, values: &BTreeMap<String, String>) -> std::io::Result<()> {
	if let Some(folder) = file.parent() {
		std::fs::create_dir_all(folder)?;
	}
	let staged = file.with_extension("conf.tmp");
	std::fs::write(&staged, render(values))?;
	std::fs::rename(staged, file)
}

fn parse(text: &str) -> BTreeMap<String, String> {
	text
		.lines()
		.filter(|line| !line.trim_start().starts_with('#'))
		.filter_map(|line| line.split_once('='))
		.map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
		.filter(|(key, _)| !key.is_empty())
		.collect()
}

fn render(values: &BTreeMap<String, String>) -> String {
	let mut out = String::from("# Thallium Files interface state. Safe to edit or delete.\n");
	for (key, value) in values {
		// One line per value: anything after a newline would read as a key.
		let value = value.replace(['\n', '\r'], " ");
		let _ = writeln!(out, "{key} = {value}");
	}
	out
}

#[cfg(test)]
mod tests {
	use std::collections::BTreeMap;

	use super::{load, parse, render, save};

	#[test]
	fn values_round_trip_through_the_file() {
		let folder = std::env::temp_dir().join(format!("thallium-ui-state-{}", std::process::id()));
		let file = folder.join("thallium").join("files.conf");
		let mut values = BTreeMap::new();
		values.insert("sidebar.open".to_owned(), "false".to_owned());
		values.insert("search.recent".to_owned(), "budget\t*.toml".to_owned());
		save(&file, &values).expect("save");
		assert_eq!(load(&file), values);
		let _ = std::fs::remove_dir_all(folder);
	}

	#[test]
	fn comments_blank_lines_and_newlines_are_handled() {
		let parsed = parse("# note\n\nkey = a = b\n = lost\nother=x\n");
		assert_eq!(parsed.get("key").map(String::as_str), Some("a = b"));
		assert_eq!(parsed.get("other").map(String::as_str), Some("x"));
		assert_eq!(parsed.len(), 2);
		let mut values = BTreeMap::new();
		values.insert("k".to_owned(), "one\ntwo".to_owned());
		assert!(render(&values).contains("k = one two\n"));
	}
}
