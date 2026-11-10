//! Renaming many entries at once by a rule, as Thunar's Bulk Rename does.
//!
//! The new names are worked out first and checked as a whole, then applied
//! in two phases through temporary names, so swapping two names, or shifting
//! a numbered run by one, never collides with itself.

use std::{collections::HashSet, fs, path::{Path, PathBuf}};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rule {
	/// Replace every occurrence of `find` in the name.
	Replace { find: String, with: String, ignore_case: bool },
	/// `pattern` with its run of `#` replaced by a counter from `start`,
	/// zero-padded to the run's length; the extension is kept.
	Number { pattern: String, start: u32 },
	/// Text added before and after the name, before the extension.
	Affix { prefix: String, suffix: String },
	/// Change the case of the name, extension included.
	Case(Case),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
	Lower,
	Upper,
	Title,
}

/// The new name of each of `names`, in order, or why the rule cannot apply.
/// `taken` are the other names in the folder, which must not be collided with.
pub fn preview(
	names: &[String],
	rule: &Rule,
	taken: &HashSet<String>,
) -> Result<Vec<String>, String> {
	let renamed: Vec<String> =
		names.iter().enumerate().map(|(at, name)| apply(name, at, rule)).collect();
	let mut seen = HashSet::new();
	for (old, new) in names.iter().zip(&renamed) {
		if new.is_empty() || new == "." || new == ".." || new.contains('/') {
			return Err(format!("“{old}” would get an invalid name"));
		}
		if !seen.insert(new.clone()) {
			return Err(format!("more than one entry would be named “{new}”"));
		}
		if new != old && taken.contains(new) && !names.contains(new) {
			return Err(format!("“{new}” already exists"));
		}
	}
	Ok(renamed)
}

fn apply(name: &str, index: usize, rule: &Rule) -> String {
	let (stem, extension) = split_extension(name);
	match rule {
		Rule::Replace { find, .. } if find.is_empty() => name.to_owned(),
		Rule::Replace { find, with, ignore_case: false } => name.replace(find.as_str(), with),
		Rule::Replace { find, with, ignore_case: true } => replace_ignoring_case(name, find, with),
		Rule::Number { pattern, start } => {
			let width = pattern.chars().filter(|c| *c == '#').count().max(1);
			let number = format!("{:0width$}", u64::from(*start) + index as u64);
			let numbered = match pattern.find('#') {
				Some(first) => {
					let run_end =
						pattern[first..].find(|c| c != '#').map_or(pattern.len(), |offset| first + offset);
					format!("{}{number}{}", &pattern[..first], &pattern[run_end..])
				}
				None => format!("{pattern}{number}"),
			};
			format!("{numbered}{extension}")
		}
		Rule::Affix { prefix, suffix } => format!("{prefix}{stem}{suffix}{extension}"),
		Rule::Case(Case::Lower) => name.to_lowercase(),
		Rule::Case(Case::Upper) => name.to_uppercase(),
		Rule::Case(Case::Title) => {
			let mut out = String::with_capacity(name.len());
			let mut start = true;
			for c in name.chars() {
				if start {
					out.extend(c.to_uppercase());
				} else {
					out.extend(c.to_lowercase());
				}
				start = !c.is_alphanumeric();
			}
			out
		}
	}
}

/// `("photo", ".jpg")`; a leading dot is a hidden name, not an extension.
fn split_extension(name: &str) -> (&str, &str) {
	match name.rfind('.') {
		Some(dot) if dot > 0 => name.split_at(dot),
		_ => (name, ""),
	}
}

fn replace_ignoring_case(name: &str, find: &str, with: &str) -> String {
	let lower_name = name.to_lowercase();
	let lower_find = find.to_lowercase();
	// Lowercasing can change byte lengths outside ASCII; fall back to an exact
	// replace rather than slice at the wrong place.
	if lower_name.len() != name.len() || lower_find.len() != find.len() {
		return name.replace(find, with);
	}
	let mut out = String::new();
	let mut rest = 0;
	while let Some(found) = lower_name[rest..].find(&lower_find) {
		out.push_str(&name[rest..rest + found]);
		out.push_str(with);
		rest += found + find.len();
	}
	out.push_str(&name[rest..]);
	out
}

/// Rename each `(from, to)` pair within `folder`, through temporary names.
/// Returns the pairs as full paths, for undo.
pub fn apply_all(
	folder: &Path,
	pairs: &[(String, String)],
) -> Result<Vec<(PathBuf, PathBuf)>, String> {
	let changing: Vec<&(String, String)> = pairs.iter().filter(|(from, to)| from != to).collect();
	let staged: Vec<PathBuf> = changing
		.iter()
		.enumerate()
		.map(|(at, _)| folder.join(format!(".thallium-rename-{}-{at}", std::process::id())))
		.collect();
	for ((from, _), temporary) in changing.iter().zip(&staged) {
		fs::rename(folder.join(from), temporary).map_err(|error| format!("{from}: {error}"))?;
	}
	let mut done = Vec::new();
	for ((from, to), temporary) in changing.iter().zip(&staged) {
		let target = folder.join(to);
		if fs::symlink_metadata(&target).is_ok() {
			// Something appeared under the name meanwhile: put this one back.
			let _ = fs::rename(temporary, folder.join(from));
			return Err(format!("“{to}” already exists"));
		}
		fs::rename(temporary, &target).map_err(|error| format!("{to}: {error}"))?;
		done.push((folder.join(from), target));
	}
	Ok(done)
}

#[cfg(test)]
mod tests {
	use std::{collections::HashSet, fs};

	use super::{Case, Rule, apply_all, preview};

	fn names(list: &[&str]) -> Vec<String> { list.iter().map(|name| (*name).to_owned()).collect() }

	#[test]
	fn rules_rename_as_expected() {
		let taken = HashSet::new();
		let files = names(&["IMG_001.JPG", "IMG_002.JPG", ".hidden"]);
		let numbered =
			preview(&files[..2], &Rule::Number { pattern: "Trip-###".into(), start: 9 }, &taken);
		assert_eq!(numbered.unwrap(), ["Trip-009.JPG", "Trip-010.JPG"]);
		let replaced = preview(
			&files[..2],
			&Rule::Replace {
				find:        "img_".into(),
				with:        "photo ".into(),
				ignore_case: true,
			},
			&taken,
		);
		assert_eq!(replaced.unwrap(), ["photo 001.JPG", "photo 002.JPG"]);
		let affixed =
			preview(&files, &Rule::Affix { prefix: "x-".into(), suffix: "-y".into() }, &taken);
		assert_eq!(affixed.unwrap(), ["x-IMG_001-y.JPG", "x-IMG_002-y.JPG", "x-.hidden-y"]);
		let titled = preview(&names(&["my holiday notes.txt"]), &Rule::Case(Case::Title), &taken);
		assert_eq!(titled.unwrap(), ["My Holiday Notes.Txt"]);
	}

	#[test]
	fn clashes_are_caught_before_anything_moves() {
		let files = names(&["a.txt", "b.txt"]);
		let same = preview(
			&files,
			&Rule::Replace { find: "a".into(), with: "b".into(), ignore_case: false },
			&HashSet::new(),
		);
		assert!(same.unwrap_err().contains("more than one"));
		let taken = HashSet::from(["x-a.txt".to_owned()]);
		let existing =
			preview(&files, &Rule::Affix { prefix: "x-".into(), suffix: String::new() }, &taken);
		assert!(existing.unwrap_err().contains("already exists"));
	}

	#[test]
	fn swapping_names_does_not_collide() {
		let root = tempfile::tempdir().expect("root");
		fs::write(root.path().join("one"), b"1").expect("seed");
		fs::write(root.path().join("two"), b"2").expect("seed");
		let pairs = [("one".to_owned(), "two".to_owned()), ("two".to_owned(), "one".to_owned())];
		let done = apply_all(root.path(), &pairs).expect("swap");
		assert_eq!(done.len(), 2);
		assert_eq!(fs::read(root.path().join("one")).expect("one"), b"2");
		assert_eq!(fs::read(root.path().join("two")).expect("two"), b"1");
	}
}
