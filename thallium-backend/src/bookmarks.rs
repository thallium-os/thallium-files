//! Bookmarked folders, kept in the GTK bookmarks file so Thunar, Nautilus
//! and file choosers all see the same list.
//!
//! Each line is a URI, optionally followed by a space and a label. Lines
//! this module does not understand, such as remote URIs, are kept as they
//! are when the file is rewritten.

use std::{fs, path::PathBuf};

use crate::uri;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bookmark {
	pub path:  Vec<u8>,
	/// The label to show: the one written in the file, or the folder name.
	pub label: String,
}

/// The local folders bookmarked, in file order.
pub fn read() -> Vec<Bookmark> { parse(&fs::read_to_string(file()).unwrap_or_default()) }

/// Bookmark `path` at the end of the list, unless it is already there.
pub fn add(path: &[u8]) -> Result<(), String> {
	let text = fs::read_to_string(file()).unwrap_or_default();
	if parse(&text).iter().any(|bookmark| bookmark.path == path) {
		return Ok(());
	}
	let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
	lines.push(uri::file_uri(path));
	write(&lines)
}

/// Forget the bookmark for `path`.
pub fn remove(path: &[u8]) -> Result<(), String> {
	let text = fs::read_to_string(file()).unwrap_or_default();
	let lines: Vec<String> = text
		.lines()
		.filter(|line| line_path(line).as_deref() != Some(path))
		.map(str::to_owned)
		.collect();
	write(&lines)
}

/// Give the bookmark for `path` a label; an empty label goes back to the
/// folder name.
pub fn rename(path: &[u8], label: &str) -> Result<(), String> {
	let text = fs::read_to_string(file()).unwrap_or_default();
	let lines: Vec<String> = text
		.lines()
		.map(|line| {
			if line_path(line).as_deref() != Some(path) {
				return line.to_owned();
			}
			let target = line.split_once(' ').map_or(line, |(target, _)| target);
			let label = label.trim();
			if label.is_empty() { target.to_owned() } else { format!("{target} {label}") }
		})
		.collect();
	write(&lines)
}

fn parse(text: &str) -> Vec<Bookmark> {
	text
		.lines()
		.filter_map(|line| {
			let path = line_path(line)?;
			let label = match line.split_once(' ') {
				Some((_, label)) if !label.trim().is_empty() => label.trim().to_owned(),
				_ => {
					let name =
						path.rsplit(|byte| *byte == b'/').find(|part| !part.is_empty()).unwrap_or(b"/");
					String::from_utf8_lossy(name).into_owned()
				}
			};
			Some(Bookmark { path, label })
		})
		.collect()
}

fn line_path(line: &str) -> Option<Vec<u8>> {
	let target = line.split_once(' ').map_or(line, |(target, _)| target);
	uri::file_path(target)
}

fn write(lines: &[String]) -> Result<(), String> {
	let path = file();
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).map_err(|error| error.to_string())?;
	}
	let mut text = lines.join("\n");
	if !text.is_empty() {
		text.push('\n');
	}
	fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))
}

fn file() -> PathBuf {
	let config = std::env::var_os("XDG_CONFIG_HOME")
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
	config.join("gtk-3.0/bookmarks")
}

#[cfg(test)]
mod tests {
	use super::parse;

	#[test]
	fn labels_come_from_the_file_or_the_folder() {
		let marks =
			parse("file:///home/a/Assets%20(ds) Assets\nfile:///home/a/Projects\nsftp://host/x Remote\n");
		assert_eq!(marks.len(), 2, "remote lines are not local folders");
		assert_eq!(marks[0].path, b"/home/a/Assets (ds)");
		assert_eq!(marks[0].label, "Assets");
		assert_eq!(marks[1].label, "Projects");
	}
}
