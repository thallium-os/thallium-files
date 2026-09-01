//! The desktop's own list of recently used files.
//!
//! Every application on the system records what it opened in the freedesktop
//! recent-files list, so reading that is the difference between "files this
//! window has seen" and "files you were working on". Nothing is written back:
//! browsing a folder is not using a file, and claiming otherwise would corrupt
//! a list the rest of the desktop depends on.

use std::{fs, path::{Path, PathBuf}};

/// Entries returned. The list on disk runs to hundreds; nobody scrolls that far
/// looking for something they touched recently.
const MAX_ENTRIES: usize = 100;

/// One recently used file.
#[derive(Clone)]
pub struct Recent {
	pub path:    PathBuf,
	pub name:    String,
	/// The RFC 3339 stamp as the desktop wrote it, used only for ordering.
	pub visited: String,
}

/// Read the recent-files list, newest first.
///
/// Entries whose file has since been removed are dropped: offering to open
/// something that is no longer there is worse than not listing it.
#[must_use]
pub fn read(home: &Path) -> Vec<Recent> {
	let source = home.join(".local/share/recently-used.xbel");
	let Ok(text) = fs::read_to_string(&source) else { return Vec::new() };

	let mut found: Vec<Recent> = Vec::new();
	for element in text.split("<bookmark ").skip(1) {
		let Some(href) = attribute(element, "href") else { continue };
		let Some(encoded) = href.strip_prefix("file://") else { continue };
		let path = PathBuf::from(percent_decode(encoded));
		if !path.is_file() {
			continue;
		}
		let name =
			path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
		if name.is_empty() {
			continue;
		}
		found.push(Recent {
			path,
			name,
			visited: attribute(element, "visited").unwrap_or_default().to_owned(),
		});
	}

	// The stamps are RFC 3339 in UTC, all written the same way, so ordering
	// them as text orders them as time without parsing any of them.
	found.sort_unstable_by(|left, right| right.visited.cmp(&left.visited));
	found.dedup_by(|left, right| left.path == right.path);
	found.truncate(MAX_ENTRIES);
	found
}

/// The value of `name="..."` in one element's opening tag.
fn attribute<'a>(element: &'a str, name: &str) -> Option<&'a str> {
	let key = format!("{name}=\"");
	let start = element.find(&key)? + key.len();
	let rest = &element[start..];
	let end = rest.find('"')?;
	Some(&rest[..end])
}

/// Decode the percent-escapes the bookmark file stores paths in.
fn percent_decode(text: &str) -> String {
	let bytes = text.as_bytes();
	let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
	let mut at = 0;
	while at < bytes.len() {
		if bytes[at] == b'%'
			&& at + 2 < bytes.len()
			&& let (Some(high), Some(low)) =
				((bytes[at + 1] as char).to_digit(16), (bytes[at + 2] as char).to_digit(16))
		{
			out.push((high * 16 + low) as u8);
			at += 3;
			continue;
		}
		out.push(bytes[at]);
		at += 1;
	}
	String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
	use std::{fs, path::PathBuf};

	use super::{attribute, percent_decode, read};

	#[test]
	fn an_attribute_is_read_from_the_opening_tag() {
		let element = r#"href="file:///a/b.txt" added="1" visited="2">"#;
		assert_eq!(attribute(element, "href"), Some("file:///a/b.txt"));
		assert_eq!(attribute(element, "visited"), Some("2"));
		assert_eq!(attribute(element, "absent"), None);
	}

	#[test]
	fn escapes_in_a_path_are_decoded() {
		assert_eq!(percent_decode("/a/b%20c/d%2Be.txt"), "/a/b c/d+e.txt");
		assert_eq!(percent_decode("/plain/path"), "/plain/path");
		// A stray percent is left alone rather than swallowing what follows.
		assert_eq!(percent_decode("/a/100%"), "/a/100%");
	}

	#[test]
	fn entries_are_newest_first_and_skip_files_that_are_gone() {
		let home = std::env::temp_dir().join(format!("thallium-recents-{}", std::process::id()));
		fs::create_dir_all(home.join(".local/share")).expect("home");
		let present = home.join("kept file.txt");
		fs::write(&present, b"x").expect("file");
		let missing = home.join("gone.txt");

		let list = format!(
			r#"<?xml version="1.0"?><xbel>
			<bookmark href="file://{}" visited="2020-01-01T00:00:00Z"></bookmark>
			<bookmark href="file://{}" visited="2026-01-01T00:00:00Z"></bookmark>
			</xbel>"#,
			missing.display(),
			present.display().to_string().replace(' ', "%20"),
		);
		fs::write(home.join(".local/share/recently-used.xbel"), list).expect("xbel");

		let found = read(&home);
		assert_eq!(found.len(), 1, "the missing file must not be listed");
		assert_eq!(found[0].path, PathBuf::from(&present));
		assert_eq!(found[0].name, "kept file.txt");

		let _ = fs::remove_dir_all(&home);
	}

	#[test]
	fn a_home_without_a_recents_file_yields_nothing() {
		assert!(read(&PathBuf::from("/nonexistent/home")).is_empty());
	}
}
