//! Installed applications, which of them open a type, and launching them.
//!
//! Follows the freedesktop Desktop Entry and MIME Applications
//! Associations specifications: desktop files are found on the XDG data
//! path, the first file with a given id wins, and `mimeapps.list` files are
//! read in precedence order with the user's own first.

use std::{collections::{HashMap, HashSet}, ffi::OsStr, fs, os::unix::{ffi::OsStrExt, process::CommandExt}, path::{Path, PathBuf}, process::{Command, Stdio}};

/// One launchable desktop entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct App {
	/// Desktop file id, such as `org.gnome.TextEditor.desktop`.
	pub id:       String,
	pub name:     String,
	pub exec:     String,
	pub icon:     String,
	pub terminal: bool,
	pub mimes:    Vec<String>,
	/// Hidden from menus, though it may still open files it declares.
	pub hidden:   bool,
	pub path:     PathBuf,
}

/// Every application on the XDG data path, by name.
pub fn installed() -> Vec<App> {
	let mut seen = HashSet::new();
	let mut apps = Vec::new();
	for root in data_dirs() {
		let folder = root.join("applications");
		let mut found = Vec::new();
		collect_desktop_files(&folder, &folder, &mut found);
		for (id, path) in found {
			if !seen.insert(id.clone()) {
				continue;
			}
			if let Some(app) = fs::read_to_string(&path).ok().and_then(|text| parse(&id, &path, &text)) {
				apps.push(app);
			}
		}
	}
	apps.sort_by_key(|app| app.name.to_lowercase());
	apps
}

/// Applications that open a file whose type is `lineage[0]`, best first:
/// the default, then explicitly associated applications, then any that
/// declare one of the types. `lineage` is the type and its parents.
pub fn for_types(apps: &[App], lineage: &[String]) -> Vec<App> {
	let lists = Associations::read();
	let by_id: HashMap<&str, &App> = apps.iter().map(|app| (app.id.as_str(), app)).collect();
	let mut ordered: Vec<App> = Vec::new();
	let push = |app: &App, ordered: &mut Vec<App>| {
		if !ordered.iter().any(|known| known.id == app.id) {
			ordered.push(app.clone());
		}
	};
	for mime in lineage {
		if let Some(app) = lists.default_for(mime).and_then(|id| by_id.get(id)) {
			push(app, &mut ordered);
		}
	}
	for mime in lineage {
		for id in lists.added.get(mime).into_iter().flatten() {
			if let Some(app) = by_id.get(id.as_str())
				&& !lists.is_removed(mime, id)
			{
				push(app, &mut ordered);
			}
		}
		for app in apps {
			if app.mimes.iter().any(|declared| declared == mime) && !lists.is_removed(mime, &app.id) {
				push(app, &mut ordered);
			}
		}
	}
	ordered
}

/// The desktop id of the default application for `mime`, if one is set.
pub fn default_for(mime: &str) -> Option<String> {
	Associations::read().default_for(mime).map(str::to_owned)
}

/// Make `app_id` the default for `mime` in the user's `mimeapps.list`,
/// keeping every other line of the file as it was.
pub fn set_default(mime: &str, app_id: &str) -> Result<(), String> {
	let path = config_home().join("mimeapps.list");
	let text = fs::read_to_string(&path).unwrap_or_default();
	let updated = with_default(&text, mime, app_id);
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).map_err(|error| error.to_string())?;
	}
	fs::write(&path, updated).map_err(|error| format!("{}: {error}", path.display()))
}

/// Start `app` on `files`, detached from this process.
pub fn launch(app: &App, files: &[PathBuf]) -> Result<(), String> {
	for argv in command_lines(app, files) {
		let argv = if app.terminal { in_terminal(argv) } else { argv };
		spawn(&argv, None)?;
	}
	Ok(())
}

/// Open the user's terminal with `folder` as its working directory.
pub fn open_terminal(folder: &Path) -> Result<(), String> { spawn(&terminal(), Some(folder)) }

/// The terminal to run: `$TERMINAL`, the Debian alternative, or the first
/// common one installed.
fn terminal() -> Vec<String> {
	if let Ok(configured) = std::env::var("TERMINAL")
		&& !configured.trim().is_empty()
	{
		return split_exec(&configured);
	}
	[
		"x-terminal-emulator",
		"kitty",
		"foot",
		"alacritty",
		"wezterm",
		"konsole",
		"gnome-terminal",
		"xterm",
	]
	.into_iter()
	.find(|name| on_path(name))
	.map_or_else(|| vec!["xterm".to_owned()], |name| vec![name.to_owned()])
}

fn in_terminal(argv: Vec<String>) -> Vec<String> {
	let mut wrapped = terminal();
	wrapped.push("-e".to_owned());
	wrapped.extend(argv);
	wrapped
}

fn on_path(name: &str) -> bool {
	std::env::var_os("PATH")
		.is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

fn spawn(argv: &[String], folder: Option<&Path>) -> Result<(), String> {
	let Some((program, args)) = argv.split_first() else {
		return Err("the application has no command to run".to_owned());
	};
	let mut command = Command::new(program);
	command.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
	if let Some(folder) = folder {
		command.current_dir(folder);
	}
	// A session of its own, so closing this window does not take the
	// application with it.
	unsafe {
		command.pre_exec(|| {
			libc::setsid();
			Ok(())
		});
	}
	let mut child = command.spawn().map_err(|error| format!("{program}: {error}"))?;
	std::thread::spawn(move || child.wait());
	Ok(())
}

/// The command lines `app` runs for `files`. An Exec line that takes one
/// file (`%f`, `%u`) runs once per file.
fn command_lines(app: &App, files: &[PathBuf]) -> Vec<Vec<String>> {
	let tokens = split_exec(&app.exec);
	let single = tokens.iter().any(|token| token.contains("%f") || token.contains("%u"));
	if single && files.len() > 1 {
		return files.iter().map(|file| expand(app, &tokens, std::slice::from_ref(file))).collect();
	}
	vec![expand(app, &tokens, files)]
}

/// Substitute the field codes of one Exec line.
fn expand(app: &App, tokens: &[String], files: &[PathBuf]) -> Vec<String> {
	let paths: Vec<String> = files.iter().map(|file| file.to_string_lossy().into_owned()).collect();
	let uris: Vec<String> =
		files.iter().map(|file| crate::uri::file_uri(file.as_os_str().as_bytes())).collect();
	let mut argv = Vec::new();
	for token in tokens {
		match token.as_str() {
			"%F" => argv.extend(paths.iter().cloned()),
			"%U" => argv.extend(uris.iter().cloned()),
			"%i" if !app.icon.is_empty() => argv.extend(["--icon".to_owned(), app.icon.clone()]),
			"%i" => {}
			_ => {
				let mut out = String::new();
				let mut chars = token.chars();
				while let Some(c) = chars.next() {
					if c != '%' {
						out.push(c);
						continue;
					}
					match chars.next() {
						Some('%') => out.push('%'),
						Some('f') => out.push_str(paths.first().map_or("", String::as_str)),
						Some('u') => out.push_str(uris.first().map_or("", String::as_str)),
						Some('c') => out.push_str(&app.name),
						Some('k') => out.push_str(&app.path.to_string_lossy()),
						// Deprecated or list codes inside a word expand to nothing.
						_ => {}
					}
				}
				if !out.is_empty() || !token.contains('%') {
					argv.push(out);
				}
			}
		}
	}
	argv
}

/// Split an Exec value into arguments, honouring its double-quote rules.
fn split_exec(exec: &str) -> Vec<String> {
	let mut args = Vec::new();
	let mut current = String::new();
	let mut quoted = false;
	let mut started = false;
	let mut chars = exec.chars();
	while let Some(c) = chars.next() {
		match c {
			'"' => {
				quoted = !quoted;
				started = true;
			}
			'\\' if quoted => {
				if let Some(next) = chars.next() {
					current.push(next);
				}
			}
			c if c.is_whitespace() && !quoted => {
				if started {
					args.push(std::mem::take(&mut current));
					started = false;
				}
			}
			c => {
				current.push(c);
				started = true;
			}
		}
	}
	if started {
		args.push(current);
	}
	args
}

/// The `[Desktop Entry]` group of a desktop file, or `None` for entries
/// that are not applications or must be ignored.
fn parse(id: &str, path: &Path, text: &str) -> Option<App> {
	let mut in_entry = false;
	let mut fields: HashMap<&str, &str> = HashMap::new();
	for line in text.lines() {
		let line = line.trim();
		if line.starts_with('[') {
			in_entry = line == "[Desktop Entry]";
			continue;
		}
		if !in_entry || line.starts_with('#') {
			continue;
		}
		if let Some((key, value)) = line.split_once('=') {
			fields.entry(key.trim()).or_insert(value.trim());
		}
	}
	let yes = |key: &str| fields.get(key).is_some_and(|value| *value == "true");
	if fields.get("Type").copied() != Some("Application") || yes("Hidden") {
		return None;
	}
	let exec = fields.get("Exec")?.to_string();
	Some(App {
		id: id.to_owned(),
		name: fields
			.get("Name")
			.map_or_else(|| id.trim_end_matches(".desktop").to_owned(), |name| (*name).to_owned()),
		exec,
		icon: fields.get("Icon").map(|icon| (*icon).to_owned()).unwrap_or_default(),
		terminal: yes("Terminal"),
		mimes: fields
			.get("MimeType")
			.map(|list| list.split(';').filter(|mime| !mime.is_empty()).map(str::to_owned).collect())
			.unwrap_or_default(),
		hidden: yes("NoDisplay"),
		path: path.to_path_buf(),
	})
}

/// Desktop files under `folder`, with ids that turn subfolders into `-`.
fn collect_desktop_files(root: &Path, folder: &Path, found: &mut Vec<(String, PathBuf)>) {
	let Ok(entries) = fs::read_dir(folder) else { return };
	let mut entries: Vec<_> = entries.filter_map(Result::ok).map(|entry| entry.path()).collect();
	entries.sort();
	for path in entries {
		if path.is_dir() {
			collect_desktop_files(root, &path, found);
		} else if path.extension() == Some(OsStr::new("desktop"))
			&& let Ok(relative) = path.strip_prefix(root)
		{
			found.push((relative.to_string_lossy().replace('/', "-"), path));
		}
	}
}

/// What the `mimeapps.list` files say, merged in precedence order.
#[derive(Default)]
struct Associations {
	defaults: HashMap<String, Vec<String>>,
	added:    HashMap<String, Vec<String>>,
	removed:  HashMap<String, Vec<String>>,
}

impl Associations {
	fn read() -> Self {
		let mut lists = Self::default();
		let config = std::iter::once(config_home()).chain(config_dirs());
		let data = data_dirs().into_iter().map(|dir| dir.join("applications"));
		for folder in config.chain(data) {
			for name in ["mimeapps.list", "defaults.list"] {
				if let Ok(text) = fs::read_to_string(folder.join(name)) {
					lists.merge(&text);
				}
			}
		}
		lists
	}

	fn merge(&mut self, text: &str) {
		let mut group = "";
		for line in text.lines() {
			let line = line.trim();
			if line.starts_with('[') {
				group = line;
				continue;
			}
			let Some((mime, ids)) = line.split_once('=') else { continue };
			let target = match group {
				"[Default Applications]" => &mut self.defaults,
				"[Added Associations]" => &mut self.added,
				"[Removed Associations]" => &mut self.removed,
				_ => continue,
			};
			let known = target.entry(mime.trim().to_owned()).or_default();
			for id in ids.split(';').map(str::trim).filter(|id| !id.is_empty()) {
				if !known.iter().any(|existing| existing == id) {
					known.push(id.to_owned());
				}
			}
		}
	}

	fn default_for(&self, mime: &str) -> Option<&str> {
		self.defaults.get(mime)?.first().map(String::as_str)
	}

	fn is_removed(&self, mime: &str, id: &str) -> bool {
		self.removed.get(mime).is_some_and(|ids| ids.iter().any(|removed| removed == id))
	}
}

/// `text` with `[Default Applications]` saying `mime=app_id`.
fn with_default(text: &str, mime: &str, app_id: &str) -> String {
	let entry = format!("{mime}={app_id};");
	let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
	let Some(header) = lines.iter().position(|line| line.trim() == "[Default Applications]") else {
		if !lines.is_empty() && !lines.last().is_some_and(|line| line.trim().is_empty()) {
			lines.push(String::new());
		}
		lines.push("[Default Applications]".to_owned());
		lines.push(entry);
		return lines.join("\n") + "\n";
	};
	let end = lines[header + 1..]
		.iter()
		.position(|line| line.trim().starts_with('['))
		.map_or(lines.len(), |offset| header + 1 + offset);
	let existing = lines[header + 1..end]
		.iter()
		.position(|line| line.split_once('=').is_some_and(|(key, _)| key.trim() == mime));
	match existing {
		Some(offset) => lines[header + 1 + offset] = entry,
		None => {
			// After the group's last entry, before any blank separator line.
			let mut at = end;
			while at > header + 1 && lines[at - 1].trim().is_empty() {
				at -= 1;
			}
			lines.insert(at, entry);
		}
	}
	lines.join("\n") + "\n"
}

fn home() -> PathBuf { std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default() }

fn config_home() -> PathBuf {
	std::env::var_os("XDG_CONFIG_HOME").map_or_else(|| home().join(".config"), PathBuf::from)
}

fn config_dirs() -> Vec<PathBuf> {
	std::env::var("XDG_CONFIG_DIRS")
		.ok()
		.filter(|dirs| !dirs.is_empty())
		.unwrap_or_else(|| "/etc/xdg".to_owned())
		.split(':')
		.map(PathBuf::from)
		.collect()
}

/// User data first, then the system data directories.
fn data_dirs() -> Vec<PathBuf> {
	let user =
		std::env::var_os("XDG_DATA_HOME").map_or_else(|| home().join(".local/share"), PathBuf::from);
	let system = std::env::var("XDG_DATA_DIRS")
		.ok()
		.filter(|dirs| !dirs.is_empty())
		.unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
	std::iter::once(user).chain(system.split(':').map(PathBuf::from)).collect()
}

#[cfg(test)]
mod tests {
	use std::path::PathBuf;

	use super::{App, Associations, command_lines, parse, split_exec, with_default};

	fn app(exec: &str) -> App {
		App {
			id:       "viewer.desktop".into(),
			name:     "Viewer".into(),
			exec:     exec.into(),
			icon:     "viewer".into(),
			terminal: false,
			mimes:    Vec::new(),
			hidden:   false,
			path:     PathBuf::from("/usr/share/applications/viewer.desktop"),
		}
	}

	#[test]
	fn exec_lines_split_on_quotes_and_expand_field_codes() {
		assert_eq!(split_exec(r#"sh -c "echo \"hi\" there" %U"#), [
			"sh",
			"-c",
			r#"echo "hi" there"#,
			"%U"
		]);
		let files = [PathBuf::from("/a b/one.txt"), PathBuf::from("/two.txt")];
		assert_eq!(command_lines(&app("viewer %F"), &files), [vec![
			"viewer",
			"/a b/one.txt",
			"/two.txt"
		]]);
		assert_eq!(command_lines(&app("viewer %U"), &files)[0][1], "file:///a%20b/one.txt");
		assert_eq!(command_lines(&app("viewer %f"), &files).len(), 2, "%f runs once per file");
		assert_eq!(command_lines(&app("viewer %i --name=%c %d"), &[]), [vec![
			"viewer",
			"--icon",
			"viewer",
			"--name=Viewer"
		]]);
	}

	#[test]
	fn only_visible_applications_are_parsed() {
		let text = "[Desktop Entry]\nType=Application\nName=Edit\nExec=edit %f\nMimeType=text/plain;\n\n[Desktop Action new]\nName=Other\nExec=other\n";
		let app = parse("edit.desktop", "/x/edit.desktop".as_ref(), text).expect("an application");
		assert_eq!(app.name, "Edit");
		assert_eq!(app.exec, "edit %f");
		assert_eq!(app.mimes, ["text/plain"]);
		assert!(parse("l.desktop", "/x".as_ref(), "[Desktop Entry]\nType=Link\nURL=x\n").is_none());
		assert!(
			parse("h.desktop", "/x".as_ref(), "[Desktop Entry]\nType=Application\nExec=x\nHidden=true\n")
				.is_none()
		);
	}

	#[test]
	fn the_first_list_wins_and_removed_associations_hold() {
		let mut lists = Associations::default();
		lists.merge("[Default Applications]\ntext/plain=mine.desktop;\n[Removed Associations]\ntext/plain=bad.desktop;\n");
		lists.merge("[Default Applications]\ntext/plain=system.desktop;\n");
		assert_eq!(lists.default_for("text/plain"), Some("mine.desktop"));
		assert!(lists.is_removed("text/plain", "bad.desktop"));
	}

	#[test]
	fn setting_a_default_keeps_the_rest_of_the_file() {
		let original = "[Default Applications]\ntext/html=web.desktop;\n\n[Added Associations]\nimage/png=view.desktop;\n";
		let updated = with_default(original, "text/plain", "edit.desktop");
		assert_eq!(
			updated,
			"[Default Applications]\ntext/html=web.desktop;\ntext/plain=edit.desktop;\n\n[Added Associations]\nimage/png=view.desktop;\n"
		);
		let replaced = with_default(&updated, "text/html", "other.desktop");
		assert!(replaced.contains("text/html=other.desktop;\n") && !replaced.contains("web.desktop"));
		assert_eq!(with_default("", "a/b", "c.desktop"), "[Default Applications]\na/b=c.desktop;\n");
	}
}
