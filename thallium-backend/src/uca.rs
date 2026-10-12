//! Custom actions, read from Thunar's `uca.xml` so actions people already
//! made keep working.
//!
//! An action offers itself when every selected entry matches one of its
//! name patterns and one of the kinds it is for. Its command runs through
//! `sh -c` in the open folder, with the field codes replaced by quoted
//! paths, as Thunar does.

use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Action {
	pub name:        String,
	pub icon:        String,
	pub command:     String,
	pub description: String,
	patterns:        Vec<String>,
	directories:     bool,
	audio:           bool,
	image:           bool,
	text:            bool,
	video:           bool,
	other:           bool,
}

/// One selected entry, as an action sees it.
pub struct Target<'a> {
	pub path:      &'a Path,
	pub directory: bool,
	pub mime:      &'a str,
}

/// The actions in the user's file, in file order.
pub fn read() -> Vec<Action> { parse(&fs::read_to_string(file()).unwrap_or_default()) }

/// The file actions are kept in, created with Thunar's example when missing
/// so there is something to edit.
pub fn file() -> PathBuf {
	let config = std::env::var_os("XDG_CONFIG_HOME")
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
	config.join("Thunar/uca.xml")
}

pub fn ensure_file() -> std::io::Result<PathBuf> {
	let path = file();
	if !path.exists() {
		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent)?;
		}
		fs::write(&path, EXAMPLE)?;
	}
	Ok(path)
}

const EXAMPLE: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n\t<icon>utilities-terminal</icon>\n\t<name>Open Terminal Here</name>\n\t<command>x-terminal-emulator --working-directory %f</command>\n\t<description>Example for a custom action</description>\n\t<patterns>*</patterns>\n\t<directories/>\n</action>\n</actions>\n";

impl Action {
	/// Whether this action is offered for `targets`.
	pub fn applies(&self, targets: &[Target<'_>]) -> bool {
		!targets.is_empty() && targets.iter().all(|target| self.matches(target))
	}

	fn matches(&self, target: &Target<'_>) -> bool {
		let name =
			target.path.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
		let named = self.patterns.iter().any(|pattern| glob(&pattern.to_lowercase(), &name));
		let kind = if target.directory {
			self.directories
		} else if target.mime.starts_with("audio/") {
			self.audio
		} else if target.mime.starts_with("image/") {
			self.image
		} else if target.mime.starts_with("video/") {
			self.video
		} else if target.mime.starts_with("text/") {
			self.text
		} else {
			self.other
		};
		named && kind
	}

	/// Run the action on `paths` from `folder`.
	pub fn run(&self, paths: &[PathBuf], folder: &Path) -> Result<(), String> {
		let line = expand(&self.command, paths);
		let mut child = Command::new("sh")
			.arg("-c")
			.arg(&line)
			.current_dir(folder)
			.stdin(Stdio::null())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.spawn()
			.map_err(|error| format!("{}: {error}", self.name))?;
		std::thread::spawn(move || child.wait());
		Ok(())
	}
}

/// Replace Thunar's field codes with shell-quoted values.
fn expand(command: &str, paths: &[PathBuf]) -> String {
	let quote = |text: &str| format!("'{}'", text.replace('\'', "'\\''"));
	let path = |path: &PathBuf| quote(&path.to_string_lossy());
	let folder = |path: &PathBuf| quote(&path.parent().unwrap_or(path).to_string_lossy());
	let name = |path: &PathBuf| {
		quote(&path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default())
	};
	let uri = |path: &PathBuf| quote(&crate::uri::file_uri(path.as_os_str().as_encoded_bytes()));
	let all =
		|each: &dyn Fn(&PathBuf) -> String| paths.iter().map(each).collect::<Vec<_>>().join(" ");
	let first = |each: &dyn Fn(&PathBuf) -> String| paths.first().map(each).unwrap_or_default();

	let mut out = String::new();
	let mut chars = command.chars();
	while let Some(c) = chars.next() {
		if c != '%' {
			out.push(c);
			continue;
		}
		match chars.next() {
			Some('f') => out.push_str(&first(&path)),
			Some('F') => out.push_str(&all(&path)),
			Some('u') => out.push_str(&first(&uri)),
			Some('U') => out.push_str(&all(&uri)),
			Some('d') => out.push_str(&first(&folder)),
			Some('D') => out.push_str(&all(&folder)),
			Some('n') => out.push_str(&first(&name)),
			Some('N') => out.push_str(&all(&name)),
			Some('%') => out.push('%'),
			Some(other) => {
				out.push('%');
				out.push(other);
			}
			None => out.push('%'),
		}
	}
	out
}

/// Shell-style `*` and `?` matching.
fn glob(pattern: &str, name: &str) -> bool {
	let (pattern, name): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
	let (mut p, mut n, mut star, mut mark) = (0, 0, None, 0);
	while n < name.len() {
		if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
			p += 1;
			n += 1;
		} else if p < pattern.len() && pattern[p] == '*' {
			star = Some(p);
			mark = n;
			p += 1;
		} else if let Some(at) = star {
			p = at + 1;
			mark += 1;
			n = mark;
		} else {
			return false;
		}
	}
	pattern[p..].iter().all(|c| *c == '*')
}

fn parse(text: &str) -> Vec<Action> {
	let mut actions = Vec::new();
	let mut rest = text;
	while let Some(start) = rest.find("<action>") {
		let after = &rest[start + "<action>".len()..];
		let Some(end) = after.find("</action>") else { break };
		let body = &after[..end];
		rest = &after[end + "</action>".len()..];
		let field = |tag: &str| element(body, tag).map(unescape).unwrap_or_default();
		let flag =
			|tag: &str| body.contains(&format!("<{tag}/>")) || body.contains(&format!("<{tag}>"));
		let action = Action {
			name:        field("name"),
			icon:        field("icon"),
			command:     field("command"),
			description: field("description"),
			patterns:    field("patterns")
				.split(';')
				.map(str::trim)
				.filter(|p| !p.is_empty())
				.map(str::to_owned)
				.collect(),
			directories: flag("directories"),
			audio:       flag("audio-files"),
			image:       flag("image-files"),
			text:        flag("text-files"),
			video:       flag("video-files"),
			other:       flag("other-files"),
		};
		if !action.name.is_empty() && !action.command.is_empty() {
			actions.push(action);
		}
	}
	actions
}

/// The text of the first `<tag>...</tag>` in `body`. Thunar writes one
/// `<name>` per language with an `xml:lang` attribute; the untranslated one
/// comes first.
fn element<'a>(body: &'a str, tag: &str) -> Option<&'a str> {
	let open = format!("<{tag}>");
	let start = body.find(&open)? + open.len();
	let end = body[start..].find(&format!("</{tag}>"))?;
	Some(body[start..start + end].trim())
}

fn unescape(text: &str) -> String {
	text
		.replace("&lt;", "<")
		.replace("&gt;", ">")
		.replace("&quot;", "\"")
		.replace("&apos;", "'")
		.replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
	use std::path::{Path, PathBuf};

	use super::{Target, expand, glob, parse};

	const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<actions>
<action>
	<icon>utilities-terminal</icon>
	<name>Open Terminal Here</name>
	<command>exo-open --working-directory %f</command>
	<patterns>*</patterns>
	<directories/>
</action>
<action>
	<name>Shrink &amp; convert</name>
	<command>convert %F -resize 50%% out.png</command>
	<patterns>*.png;*.JPG</patterns>
	<image-files/>
</action>
</actions>"#;

	#[test]
	fn actions_apply_by_pattern_and_kind() {
		let actions = parse(SAMPLE);
		assert_eq!(actions.len(), 2);
		assert_eq!(actions[1].name, "Shrink & convert");
		let folder =
			Target { path: Path::new("/a/dir"), directory: true, mime: "inode/directory" };
		let photo =
			Target { path: Path::new("/a/p.jpg"), directory: false, mime: "image/jpeg" };
		let text =
			Target { path: Path::new("/a/p.png"), directory: false, mime: "text/plain" };
		assert!(actions[0].applies(&[folder]));
		assert!(actions[1].applies(&[photo]), "patterns ignore case");
		assert!(!actions[1].applies(&[text]), "a png name with text content is not an image");
	}

	#[test]
	fn field_codes_expand_to_quoted_values() {
		let paths = [PathBuf::from("/a b/it's.png"), PathBuf::from("/c/d.png")];
		assert_eq!(expand("x %f", &paths), r"x '/a b/it'\''s.png'");
		assert_eq!(expand("x %N in %d", &paths), r"x 'it'\''s.png' 'd.png' in '/a b'");
		assert_eq!(expand("50%%", &paths), "50%");
	}

	#[test]
	fn globs_match_like_a_shell() {
		assert!(glob("*.tar.*", "a.tar.gz"));
		assert!(glob("?.txt", "a.txt"));
		assert!(!glob("*.txt", "a.txt.bak"));
		assert!(glob("*", ""));
	}
}
