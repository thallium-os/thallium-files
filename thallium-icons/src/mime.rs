//! MIME resolution, and the icon-name chain a MIME type implies.
//!
//! Name-based lookup is tried first because it is pure string work; the
//! content sniff that reads the head of the file only runs when the name
//! says nothing, so listing a directory does not read every file in it.

use std::{collections::HashMap, path::Path, sync::Mutex};

use xdg_mime::SharedMimeInfo;

/// The type every unrecognised file falls back to.
const OCTET_STREAM: &str = "application/octet-stream";

pub struct MimeIndex {
	database: SharedMimeInfo,
	/// File name (or bare extension) to MIME type. Extensions dominate a real
	/// directory, so this collapses to a handful of entries per view.
	by_name:  Mutex<HashMap<String, String>>,
	/// Type to the types it is a subclass of, from `mime/subclasses`.
	parents:  HashMap<String, Vec<String>>,
}

impl MimeIndex {
	pub fn new() -> Self {
		Self {
			database: SharedMimeInfo::new(),
			by_name:  Mutex::new(HashMap::new()),
			parents:  read_subclasses(),
		}
	}

	/// The MIME type of `path`, as a string.
	pub fn of(&self, path: &Path, is_directory: bool) -> String {
		if is_directory {
			return "inode/directory".to_owned();
		}
		let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
			return OCTET_STREAM.to_owned();
		};
		let key = cache_key(name);
		if let Some(hit) = self.by_name.lock().ok().and_then(|cache| cache.get(&key).cloned()) {
			return hit;
		}

		let mut mime = self
			.database
			.get_mime_types_from_file_name(name)
			.into_iter()
			.map(|mime| mime.essence_str().to_owned())
			.find(|mime| mime != OCTET_STREAM);

		// Extensionless or unknown suffix: fall back to reading the head of the
		// file. Only these paths pay for I/O, and only once per name shape.
		if mime.is_none() {
			let guess = self.database.guess_mime_type().path(path).guess();
			let sniffed = guess.mime_type().essence_str().to_owned();
			if sniffed != OCTET_STREAM {
				mime = Some(sniffed);
			}
		}

		let mime = mime.unwrap_or_else(|| OCTET_STREAM.to_owned());
		// An extensionless name is cached under itself, so the sniff is not
		// repeated for the same file on every repaint.
		if let Ok(mut cache) = self.by_name.lock() {
			cache.insert(key, mime.clone());
		}
		mime
	}

	/// `mime` and every type it is a kind of, most specific first: its
	/// canonical name, its declared parents, theirs, and finally the type every
	/// regular file is. Applications registered for any of these can open it.
	pub fn lineage(&self, mime: &str) -> Vec<String> {
		let mut lineage = Vec::new();
		let mut pending = vec![mime.to_owned()];
		while let Some(next) = pending.pop() {
			let Ok(parsed) = next.parse::<mime::Mime>() else { continue };
			let canonical = self.database.unalias_mime_type(&parsed).unwrap_or(parsed);
			let name = canonical.essence_str().to_owned();
			if lineage.contains(&name) {
				continue;
			}
			lineage.push(name.clone());
			// `SharedMimeInfo::get_parents` answers only for aliases, so the
			// subclass table is read here instead.
			let mut parents = self.parents.get(&name).cloned().unwrap_or_default();
			// Breadth first: the nearest parents are tried before theirs.
			parents.reverse();
			pending.splice(0..0, parents);
		}
		if mime != "inode/directory" && !lineage.iter().any(|name| name == OCTET_STREAM) {
			lineage.push(OCTET_STREAM.to_owned());
		}
		lineage
	}

	/// Icon names to try for `mime`, most specific first.
	///
	/// `shared-mime-info` supplies the specific name and the type's declared
	/// generic icon; the `<media>-x-generic` and `text-x-generic` tails are the
	/// conventional last resorts, so a type the database says nothing about
	/// still lands on a real icon.
	pub fn icon_names(&self, mime: &str) -> Vec<String> {
		if mime == "inode/directory" {
			return vec!["inode-directory".to_owned(), "folder".to_owned()];
		}

		let mut names = Vec::new();
		if let Ok(parsed) = mime.parse::<::mime::Mime>() {
			for name in self.database.lookup_icon_names(&parsed) {
				push_unique(&mut names, name);
			}
			if let Some(generic) = self.database.lookup_generic_icon_name(&parsed) {
				push_unique(&mut names, generic);
			}
			push_unique(&mut names, format!("{}-x-generic", parsed.type_()));
		}
		push_unique(&mut names, "text-x-generic".to_owned());
		names
	}
}

impl Default for MimeIndex {
	fn default() -> Self { Self::new() }
}

/// Names that share a suffix share a MIME type, so cache by suffix where there
/// is one and by the whole name where there is not.
/// Every `mime/subclasses` table on the XDG data path, user data first.
fn read_subclasses() -> HashMap<String, Vec<String>> {
	let home = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from).or_else(|| {
		std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".local/share"))
	});
	let system = std::env::var("XDG_DATA_DIRS")
		.ok()
		.filter(|dirs| !dirs.is_empty())
		.unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
	let mut parents: HashMap<String, Vec<String>> = HashMap::new();
	for root in home.into_iter().chain(system.split(':').map(std::path::PathBuf::from)) {
		let Ok(text) = std::fs::read_to_string(root.join("mime/subclasses")) else { continue };
		for line in text.lines() {
			if let Some((child, parent)) = line.split_once(' ') {
				let known = parents.entry(child.to_owned()).or_default();
				if !known.iter().any(|existing| existing == parent) {
					known.push(parent.to_owned());
				}
			}
		}
	}
	parents
}

fn cache_key(name: &str) -> String {
	match name.rfind('.') {
		Some(dot) if dot > 0 && dot + 1 < name.len() => name[dot..].to_ascii_lowercase(),
		_ => name.to_owned(),
	}
}

fn push_unique(names: &mut Vec<String>, name: String) {
	if !names.contains(&name) {
		names.push(name);
	}
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::{MimeIndex, cache_key};

	#[test]
	fn suffixes_are_the_cache_key() {
		assert_eq!(cache_key("main.RS"), ".rs");
		assert_eq!(cache_key("README"), "README");
		assert_eq!(cache_key(".bashrc"), ".bashrc");
	}

	#[test]
	fn directories_resolve_without_touching_the_database() {
		let index = MimeIndex::new();
		assert_eq!(index.of(Path::new("/tmp/somewhere"), true), "inode/directory");
		assert_eq!(
			index.icon_names("inode/directory").first().map(String::as_str),
			Some("inode-directory")
		);
	}

	#[test]
	fn source_code_is_also_plain_text_and_a_file() {
		let index = MimeIndex::new();
		let lineage = index.lineage("text/x-csrc");
		assert_eq!(lineage.first().map(String::as_str), Some("text/x-csrc"));
		assert!(lineage.iter().any(|mime| mime == "text/plain"), "{lineage:?}");
		assert_eq!(lineage.last().map(String::as_str), Some("application/octet-stream"));
	}

	#[test]
	fn unknown_types_still_end_on_a_generic_icon() {
		let index = MimeIndex::new();
		let names = index.icon_names("application/x-not-a-real-type");
		assert_eq!(names.last().map(String::as_str), Some("text-x-generic"));
	}
}
