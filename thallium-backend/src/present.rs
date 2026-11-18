//! What the window shows about files, worked out here rather than in QML:
//! type tags and kind names, sizes and dates, the order rows are listed in,
//! breadcrumbs, folder composition and the sidebar's folder tree.
//!
//! Everything here is plain data in and text out, so it is tested without
//! Qt. Lists the window draws travel as small JSON documents.

use std::{cmp::Ordering, collections::HashMap, fmt::Write as _};

/// The broad group a file belongs to, as the Kind sort and the type tags
/// see it.
#[must_use]
pub fn kind_group(name: &str, folder: bool) -> &'static str {
	if folder {
		return "Folders";
	}
	match extension(name).as_str() {
		"rs" | "js" | "ts" | "jsx" | "tsx" | "py" | "c" | "h" | "cpp" | "hpp" | "go" | "java"
		| "kt" | "rb" | "sh" | "bash" | "fish" | "qml" | "sql" | "json" | "toml" | "yaml" | "yml"
		| "conf" | "ini" | "lock" => "Code",
		"txt" | "md" | "pdf" | "odt" | "doc" | "docx" | "rtf" | "csv" | "ods" | "xlsx" => "Documents",
		"png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "mp4" | "mkv" | "webm" | "mov"
		| "mp3" | "flac" | "wav" | "ogg" => "Media",
		"zip" | "tar" | "gz" | "xz" | "zst" | "bz2" | "7z" | "rar" | "deb" | "rpm" => "Archives",
		_ => "Other",
	}
}

/// The Thallium 81 type tag: DIR, CODE, DOC, TXT for plain text, IMG / VID
/// / AUD for media, ZIP for archives, and TXT for anything else.
#[must_use]
pub fn type_tag(name: &str, folder: bool) -> &'static str {
	let extension = extension(name);
	match kind_group(name, folder) {
		"Folders" => "DIR",
		"Code" => "CODE",
		"Documents" if matches!(extension.as_str(), "txt" | "md") => "TXT",
		"Documents" => "DOC",
		"Media" if matches!(extension.as_str(), "mp4" | "mkv" | "webm" | "mov") => "VID",
		"Media" if matches!(extension.as_str(), "mp3" | "flac" | "wav" | "ogg") => "AUD",
		"Media" => "IMG",
		"Archives" => "ZIP",
		_ => "TXT",
	}
}

/// Which of the four composition groups a tag counts towards.
#[must_use]
pub fn bucket(tag: &str) -> &'static str {
	match tag {
		"DIR" => "DIR",
		"CODE" => "CODE",
		"DOC" => "DOC",
		_ => "TXT",
	}
}

/// What a person would call the file, for the Kind column.
#[must_use]
pub fn kind_name(name: &str, folder: bool) -> String {
	if folder {
		return "Folder".to_owned();
	}
	let extension = extension(name);
	let named = match extension.as_str() {
		"txt" => "Plain text",
		"md" => "Markdown",
		"pdf" => "PDF document",
		"odt" | "doc" | "docx" => "Document",
		"rtf" => "Rich text",
		"csv" | "ods" | "xlsx" | "xls" => "Spreadsheet",
		"odp" | "pptx" => "Presentation",
		"json" => "JSON data",
		"toml" => "TOML source",
		"yaml" | "yml" => "YAML source",
		"lock" => "Lock file",
		"sh" => "Shell script",
		"png" => "PNG image",
		"jpg" | "jpeg" => "JPEG image",
		"gif" => "GIF image",
		"webp" => "WebP image",
		"svg" => "SVG image",
		"mp4" | "mkv" | "webm" | "mov" => "Video",
		"mp3" | "flac" | "wav" | "ogg" => "Audio",
		"zip" | "tar" | "gz" | "xz" | "zst" | "7z" => "Archive",
		"deb" => "Package",
		"iso" => "Disc image",
		_ => "",
	};
	if !named.is_empty() {
		return named.to_owned();
	}
	if extension.is_empty() {
		return "File".to_owned();
	}
	let upper = extension.to_uppercase();
	if kind_group(name, false) == "Code" {
		format!("{upper} source")
	} else {
		format!("{upper} file")
	}
}

/// A byte count the way every size in the window reads: `312 B`, `1.4 KB`,
/// `118 MB`.
#[must_use]
pub fn readable_size(bytes: f64) -> String {
	const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
	let mut value = bytes.max(0.0);
	let mut at = 0;
	while value >= 1024.0 && at < UNITS.len() - 1 {
		value /= 1024.0;
		at += 1;
	}
	if value >= 100.0 || at == 0 {
		format!("{} {}", value.round(), UNITS[at])
	} else {
		format!("{value:.1} {}", UNITS[at])
	}
}

const MONTHS: [&str; 12] =
	["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// A moment broken into the parts the date texts use, in local time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Moment {
	pub year:    i32,
	/// 0 for January.
	pub month:   u32,
	pub day:     u32,
	pub hour:    u32,
	pub minute:  u32,
	/// Days since the epoch, for counting how many days apart two moments are.
	pub ordinal: i64,
}

/// `seconds` since the epoch in the machine's local time zone.
#[must_use]
pub fn local(seconds: i64) -> Option<Moment> {
	let stamp = libc::time_t::try_from(seconds).ok()?;
	// SAFETY: `localtime_r` writes only to `parts`, which lives for the call.
	let mut parts: libc::tm = unsafe { std::mem::zeroed() };
	let filled = unsafe { libc::localtime_r(&raw const stamp, &raw mut parts) };
	if filled.is_null() {
		return None;
	}
	// `c_long` is narrower than i64 on some targets.
	#[allow(clippy::useless_conversion)]
	let offset = i64::from(parts.tm_gmtoff);
	Some(Moment {
		year:    parts.tm_year + 1900,
		month:   u32::try_from(parts.tm_mon).ok()?,
		day:     u32::try_from(parts.tm_mday).ok()?,
		hour:    u32::try_from(parts.tm_hour).ok()?,
		minute:  u32::try_from(parts.tm_min).ok()?,
		ordinal: (seconds + offset).div_euclid(86_400),
	})
}

/// "Today, 09:58", "Yesterday, 23:02", "28 Oct, 16:31", or "3 Mar 2024" for
/// another year; "—" when there is no date.
#[must_use]
pub fn stamp_between(then: Option<Moment>, now: Option<Moment>) -> String {
	let (Some(then), Some(now)) = (then, now) else { return "—".to_owned() };
	let time = format!("{:02}:{:02}", then.hour, then.minute);
	let month = MONTHS[then.month as usize % 12];
	match now.ordinal - then.ordinal {
		0 => format!("Today, {time}"),
		1 => format!("Yesterday, {time}"),
		_ if then.year == now.year => format!("{} {month}, {time}", then.day),
		_ => format!("{} {month} {}", then.day, then.year),
	}
}

/// The Modified text for `seconds` since the epoch, against the clock now.
#[must_use]
pub fn stamp(seconds: f64) -> String {
	if seconds <= 0.0 {
		return "—".to_owned();
	}
	#[allow(clippy::cast_possible_truncation)]
	let then = local(seconds as i64);
	let now = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.ok()
		.and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
		.and_then(local);
	stamp_between(then, now)
}

/// Seconds since the epoch for an RFC 3339 stamp such as the desktop's
/// recent-files list writes (`2026-11-09T10:11:00Z`, or with an offset).
#[must_use]
pub fn rfc3339_seconds(text: &str) -> Option<i64> {
	let text = text.trim();
	let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
	let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
	let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
	let rest = text.get(19..)?.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
	let offset = match rest {
		"" | "Z" | "z" => 0,
		zone => {
			let sign = if zone.starts_with('-') { -1 } else { 1 };
			let hours = zone.get(1..3)?.parse::<i64>().ok()?;
			let minutes = zone.get(4..6)?.parse::<i64>().ok()?;
			sign * (hours * 3600 + minutes * 60)
		}
	};
	Some(days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
	let year = if month <= 2 { year - 1 } else { year };
	let era = year.div_euclid(400);
	let of_era = year - era * 400;
	let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
	let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
	era * 146_097 + of_cycle - 719_468
}

/// The lowercase text after the last dot, or nothing for a dotfile or a
/// name without one.
fn extension(name: &str) -> String {
	match name.rfind('.') {
		Some(dot) if dot > 0 => name[dot + 1..].to_lowercase(),
		_ => String::new(),
	}
}

/// One directory entry as the list orders it.
pub struct Listed<'a> {
	pub row:      usize,
	pub name:     &'a str,
	pub folder:   bool,
	pub modified: i64,
}

/// The rows to show, in order: hidden entries left out unless asked for,
/// only names containing `needle`, folders first, then by `key` ("name",
/// "kind" or "modified").
#[must_use]
pub fn ordered_rows(
	entries: &[Listed<'_>],
	needle: &str,
	hidden: bool,
	key: &str,
	ascending: bool,
) -> Vec<usize> {
	let needle = needle.to_lowercase();
	let mut kept: Vec<&Listed<'_>> = entries
		.iter()
		.filter(|entry| hidden || !entry.name.starts_with('.'))
		.filter(|entry| needle.is_empty() || entry.name.to_lowercase().contains(&needle))
		.collect();
	kept.sort_by(|a, b| {
		if a.folder != b.folder {
			return if a.folder { Ordering::Less } else { Ordering::Greater };
		}
		let order = match key {
			"modified" => a.modified.cmp(&b.modified),
			"kind" => kind_group(a.name, a.folder)
				.cmp(kind_group(b.name, b.folder))
				.then_with(|| by_name(a.name, b.name)),
			_ => by_name(a.name, b.name),
		};
		if ascending { order } else { order.reverse() }
	});
	kept.into_iter().map(|entry| entry.row).collect()
}

/// Names in the order a person reads them: letters regardless of case, then
/// the exact bytes to keep the order stable.
fn by_name(a: &str, b: &str) -> Ordering {
	a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// How many of the listed entries fall in each composition group, in the
/// order DIR, CODE, DOC, TXT.
#[must_use]
pub fn composition<'a>(names: impl IntoIterator<Item = (&'a str, bool)>) -> [usize; 4] {
	let mut counts = [0; 4];
	for (name, folder) in names {
		let at = match bucket(type_tag(name, folder)) {
			"DIR" => 0,
			"CODE" => 1,
			"DOC" => 2,
			_ => 3,
		};
		counts[at] += 1;
	}
	counts
}

/// The path as breadcrumbs: from Home when it lies under `home`, otherwise
/// from the filesystem root. Each crumb is (label, path).
#[must_use]
pub fn crumbs(path: &str, home: &str) -> Vec<(String, String)> {
	if path.is_empty() || path == "/" {
		return vec![("Filesystem".to_owned(), "/".to_owned())];
	}
	let under_home = !home.is_empty() && (path == home || path.starts_with(&format!("{home}/")));
	let (mut out, mut walked, rest) = if under_home {
		(vec![("Home".to_owned(), home.to_owned())], home.to_owned(), &path[home.len()..])
	} else {
		(vec![("Filesystem".to_owned(), "/".to_owned())], String::new(), path)
	};
	for part in rest.split('/').filter(|part| !part.is_empty()) {
		walked.push('/');
		walked.push_str(part);
		out.push((part.to_owned(), walked.clone()));
	}
	out
}

/// A guide of a level above that runs past a tree row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rail {
	pub show: bool,
	pub lit:  bool,
}

/// One folder in a sidebar tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeRow {
	pub path:     String,
	pub name:     String,
	pub depth:    usize,
	pub count:    i64,
	pub open:     bool,
	pub last:     bool,
	pub rails:    Vec<Rail>,
	pub here:     bool,
	pub on_path:  bool,
	/// The guide from the top of the row down to its arm is lit.
	pub top_lit:  bool,
	/// The guide carrying on below the arm is lit.
	pub rest_lit: bool,
}

/// The folders beneath `base` as tree rows. A folder is open when it was
/// opened by hand (`toggled` true), or when the current folder `here` is
/// inside it and it was not closed by hand. `children` lists a folder's
/// subfolders with their entry counts.
pub fn folder_tree(
	base: &str,
	first_depth: usize,
	here: &str,
	toggled: &HashMap<String, bool>,
	children: &mut dyn FnMut(&str) -> Vec<(String, i64)>,
) -> Vec<TreeRow> {
	let mut out = Vec::new();
	walk(base, first_depth, &[], here, toggled, children, &mut out);
	out
}

fn under(here: &str, path: &str) -> bool { here == path || here.starts_with(&format!("{path}/")) }

fn walk(
	path: &str,
	depth: usize,
	rails: &[Rail],
	here: &str,
	toggled: &HashMap<String, bool>,
	children: &mut dyn FnMut(&str) -> Vec<(String, i64)>,
	out: &mut Vec<TreeRow>,
) {
	let kids = children(path);
	let lit_to = kids.iter().rposition(|(name, _)| under(here, &format!("{path}/{name}")));
	for (at, (name, count)) in kids.iter().enumerate() {
		let child = format!("{path}/{name}");
		let last = at + 1 == kids.len();
		let open = toggled.get(&child).copied().unwrap_or_else(|| under(here, &child));
		let rest_lit = lit_to.is_some_and(|to| at < to);
		out.push(TreeRow {
			path: child.clone(),
			name: name.clone(),
			depth,
			count: *count,
			open,
			last,
			rails: rails.to_vec(),
			here: here == child,
			on_path: under(here, &child),
			top_lit: lit_to.is_some_and(|to| at <= to),
			rest_lit,
		});
		if open && depth < 12 {
			let mut deeper = rails.to_vec();
			deeper.push(Rail { show: !last, lit: rest_lit });
			walk(&child, depth + 1, &deeper, here, toggled, children, out);
		}
	}
}

/// Read a `{"path": true, …}` map of folders opened or closed by hand. Only
/// the flat shape the window writes is understood; anything else is empty.
#[must_use]
pub fn parse_toggles(text: &str) -> HashMap<String, bool> {
	let mut out = HashMap::new();
	let mut rest = text.trim().trim_start_matches('{').trim_end_matches('}');
	while let Some(start) = rest.find('"') {
		let Some((key, after)) = read_string(&rest[start..]) else { break };
		let after = after.trim_start().trim_start_matches(':').trim_start();
		let value = after.starts_with("true");
		if !value && !after.starts_with("false") {
			break;
		}
		out.insert(key, value);
		rest = after.trim_start_matches("true").trim_start_matches("false");
	}
	out
}

/// One JSON string at the start of `text`, unescaped, and what follows it.
fn read_string(text: &str) -> Option<(String, &str)> {
	let mut out = String::new();
	let mut chars = text.char_indices().skip(1);
	while let Some((at, c)) = chars.next() {
		match c {
			'"' => return Some((out, &text[at + 1..])),
			'\\' => match chars.next()?.1 {
				'n' => out.push('\n'),
				't' => out.push('\t'),
				'u' => {
					let code: String = (0..4).filter_map(|_| chars.next().map(|(_, c)| c)).collect();
					out.push(char::from_u32(u32::from_str_radix(&code, 16).ok()?)?);
				}
				other => out.push(other),
			},
			c => out.push(c),
		}
	}
	None
}

/// Text as a JSON string literal.
#[must_use]
pub fn json_string(text: &str) -> String {
	let mut out = String::with_capacity(text.len() + 2);
	out.push('"');
	for c in text.chars() {
		match c {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\n' => out.push_str("\\n"),
			'\r' => out.push_str("\\r"),
			'\t' => out.push_str("\\t"),
			c if (c as u32) < 0x20 => {
				let _ = write!(out, "\\u{:04x}", c as u32);
			}
			c => out.push(c),
		}
	}
	out.push('"');
	out
}

/// Tree rows as the JSON array the sidebar draws from.
#[must_use]
pub fn tree_json(rows: &[TreeRow]) -> String {
	let mut out = String::from("[");
	for (at, row) in rows.iter().enumerate() {
		if at > 0 {
			out.push(',');
		}
		let rails: Vec<String> = row
			.rails
			.iter()
			.map(|rail| format!("{{\"show\":{},\"lit\":{}}}", rail.show, rail.lit))
			.collect();
		let _ = write!(
			out,
			"{{\"path\":{},\"name\":{},\"depth\":{},\"count\":{},\"open\":{},\"last\":{},\"rails\":[{}],\
			 \"here\":{},\"onPath\":{},\"topLit\":{},\"restLit\":{},\"remote\":false,\"top\":false}}",
			json_string(&row.path),
			json_string(&row.name),
			row.depth,
			row.count,
			row.open,
			row.last,
			rails.join(","),
			row.here,
			row.on_path,
			row.top_lit,
			row.rest_lit,
		);
	}
	out.push(']');
	out
}

/// Breadcrumbs as a JSON array of `{label, path}`.
#[must_use]
pub fn crumbs_json(crumbs: &[(String, String)]) -> String {
	let parts: Vec<String> = crumbs
		.iter()
		.map(|(label, path)| {
			format!("{{\"label\":{},\"path\":{}}}", json_string(label), json_string(path))
		})
		.collect();
	format!("[{}]", parts.join(","))
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::{Listed, Moment, bucket, composition, crumbs, crumbs_json, folder_tree, json_string, kind_name, ordered_rows, parse_toggles, readable_size, rfc3339_seconds, stamp_between, tree_json, type_tag};

	#[test]
	fn files_get_their_tags_and_kind_names() {
		assert_eq!(type_tag("src", true), "DIR");
		assert_eq!(type_tag("Cargo.toml", false), "CODE");
		assert_eq!(type_tag("budget.xlsx", false), "DOC");
		assert_eq!(type_tag("todo.txt", false), "TXT");
		assert_eq!(type_tag("shot.PNG", false), "IMG");
		assert_eq!(type_tag("clip.mkv", false), "VID");
		assert_eq!(type_tag("song.flac", false), "AUD");
		assert_eq!(type_tag("backup.tar", false), "ZIP");
		assert_eq!(type_tag(".bashrc", false), "TXT");
		assert_eq!(bucket("IMG"), "TXT");
		assert_eq!(bucket("CODE"), "CODE");
		assert_eq!(kind_name("Cargo.toml", false), "TOML source");
		assert_eq!(kind_name("main.rs", false), "RS source");
		assert_eq!(kind_name("notes.xyz", false), "XYZ file");
		assert_eq!(kind_name("README", false), "File");
		assert_eq!(kind_name("docs", true), "Folder");
	}

	#[test]
	fn sizes_read_the_same_everywhere() {
		assert_eq!(readable_size(312.0), "312 B");
		assert_eq!(readable_size(1434.0), "1.4 KB");
		assert_eq!(readable_size(123.0 * 1024.0 * 1024.0), "123 MB");
		assert_eq!(readable_size(-5.0), "0 B");
	}

	fn moment(
		year: i32,
		month: u32,
		day: u32,
		hour: u32,
		minute: u32,
		ordinal: i64,
	) -> Option<Moment> {
		Some(Moment { year, month, day, hour, minute, ordinal })
	}

	#[test]
	fn dates_say_today_yesterday_or_the_day() {
		let now = moment(2026, 10, 9, 12, 0, 20_401);
		assert_eq!(stamp_between(moment(2026, 10, 9, 9, 58, 20_401), now), "Today, 09:58");
		assert_eq!(stamp_between(moment(2026, 10, 8, 23, 2, 20_400), now), "Yesterday, 23:02");
		assert_eq!(stamp_between(moment(2026, 9, 28, 16, 31, 20_389), now), "28 Oct, 16:31");
		assert_eq!(stamp_between(moment(2024, 2, 3, 8, 0, 19_785), now), "3 Mar 2024");
		assert_eq!(stamp_between(None, now), "—");
	}

	#[test]
	fn rfc3339_stamps_become_epoch_seconds() {
		assert_eq!(rfc3339_seconds("1970-01-01T00:00:00Z"), Some(0));
		assert_eq!(rfc3339_seconds("2026-11-09T10:11:00Z"), Some(1_794_219_060));
		assert_eq!(rfc3339_seconds("2026-11-09T15:41:00+05:30"), Some(1_794_219_060));
		assert_eq!(rfc3339_seconds("2026-11-09T10:11:00.123456Z"), Some(1_794_219_060));
		assert_eq!(rfc3339_seconds("yesterday"), None);
	}

	#[test]
	fn rows_list_folders_first_then_by_the_sort_key() {
		let entries = [
			Listed { row: 0, name: "b.txt", folder: false, modified: 3 },
			Listed { row: 1, name: "Alpha", folder: true, modified: 1 },
			Listed { row: 2, name: ".hidden", folder: false, modified: 9 },
			Listed { row: 3, name: "a.rs", folder: false, modified: 5 },
			Listed { row: 4, name: "zeta", folder: true, modified: 2 },
		];
		assert_eq!(ordered_rows(&entries, "", false, "name", true), vec![1, 4, 3, 0]);
		assert_eq!(ordered_rows(&entries, "", true, "name", false), vec![4, 1, 0, 3, 2]);
		assert_eq!(ordered_rows(&entries, "", false, "modified", true), vec![1, 4, 0, 3]);
		assert_eq!(ordered_rows(&entries, "", false, "kind", true), vec![1, 4, 3, 0]);
		assert_eq!(ordered_rows(&entries, "A", false, "name", true), vec![1, 4, 3]);
	}

	#[test]
	fn composition_counts_the_four_groups() {
		let names =
			[("src", true), ("main.rs", false), ("plan.pdf", false), ("shot.png", false), ("x", false)];
		assert_eq!(composition(names), [1, 1, 1, 2]);
	}

	#[test]
	fn crumbs_start_at_home_or_the_root() {
		let home = crumbs("/home/u/Projects/src", "/home/u");
		assert_eq!(home, vec![
			("Home".to_owned(), "/home/u".to_owned()),
			("Projects".to_owned(), "/home/u/Projects".to_owned()),
			("src".to_owned(), "/home/u/Projects/src".to_owned()),
		]);
		assert_eq!(crumbs("/etc", "/home/u")[1], ("etc".to_owned(), "/etc".to_owned()));
		assert_eq!(crumbs("/", "/home/u").len(), 1);
		assert_eq!(
			crumbs_json(&crumbs("/a", "")),
			r#"[{"label":"Filesystem","path":"/"},{"label":"a","path":"/a"}]"#
		);
	}

	#[test]
	fn the_tree_opens_along_the_current_path_and_lights_it() {
		let mut listing = |path: &str| -> Vec<(String, i64)> {
			match path {
				"/h" => vec![("a".into(), 1), ("b".into(), 2), ("c".into(), 0)],
				"/h/b" => vec![("x".into(), 0), ("y".into(), 4)],
				_ => vec![],
			}
		};
		let rows = folder_tree("/h", 1, "/h/b/x", &HashMap::new(), &mut listing);
		let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
		assert_eq!(names, ["a", "b", "x", "y", "c"]);
		assert!(rows[0].top_lit && rows[0].rest_lit && !rows[0].on_path);
		assert!(rows[1].open && rows[1].on_path && rows[1].top_lit && !rows[1].rest_lit);
		assert!(rows[2].here && rows[2].depth == 2);
		assert_eq!(rows[2].rails.len(), 1);
		assert!(rows[2].rails[0].show && !rows[2].rails[0].lit);
		assert!(rows[4].last && !rows[4].top_lit);

		let closed = parse_toggles(r#"{"/h/b": false, "/h/a": true}"#);
		let rows = folder_tree("/h", 1, "/h/b/x", &closed, &mut listing);
		let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
		assert_eq!(names, ["a", "b", "c"]);
		assert!(tree_json(&rows).starts_with(r#"[{"path":"/h/a","name":"a","depth":1"#));
	}

	#[test]
	fn json_strings_escape_what_they_must() {
		assert_eq!(json_string("a\"b\\c\nd"), r#""a\"b\\c\nd""#);
		let parsed = parse_toggles(r#"{"/x \"y\"": true, "/zé": false}"#);
		assert_eq!(parsed.get("/x \"y\""), Some(&true));
		assert_eq!(parsed.get("/zé"), Some(&false));
		assert!(parse_toggles("{}").is_empty());
	}
}
