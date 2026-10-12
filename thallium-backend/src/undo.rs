//! Undoing the last file operations, as Ctrl+Z does in Thunar.
//!
//! A step is recorded when an operation is submitted, from what it was asked
//! to do. Undoing never deletes: what an operation made goes to the trash.
//! Anything that cannot be traced back for certain, such as a copy onto a
//! name that was already taken, is not recorded rather than guessed at.

use std::{os::unix::ffi::OsStrExt, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

use thallium_actions::Command;
use thallium_fs_core::{BackendId, ConflictPolicy, Location, NativePath};

/// How many steps are remembered.
const DEPTH: usize = 50;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
	/// `to` was `from` before a rename.
	Renamed { from: PathBuf, to: PathBuf },
	/// These did not exist before.
	Made(Vec<PathBuf>),
	/// Each entry moved from the first path to the second.
	Moved(Vec<(PathBuf, PathBuf)>),
	/// These went to the trash at or after `since` (seconds).
	Trashed { originals: Vec<PathBuf>, since: i64 },
	/// A bulk rename within one folder: each entry was the first path.
	RenamedMany(Vec<(PathBuf, PathBuf)>),
}

/// What undoing a step takes.
pub enum Undo {
	Run(Vec<Command>),
	Restore {
		originals: Vec<PathBuf>,
		since:     i64,
	},
	/// Rename each second path back to the first, all at once.
	RenameBack(Vec<(PathBuf, PathBuf)>),
}

#[derive(Default)]
pub struct History(Vec<Step>);

impl History {
	pub fn record(&mut self, step: Step) {
		if self.0.len() == DEPTH {
			self.0.remove(0);
		}
		self.0.push(step);
	}

	pub fn pop(&mut self) -> Option<Step> { self.0.pop() }

	pub fn is_empty(&self) -> bool { self.0.is_empty() }
}

/// The step `command` will be undone by, judged before it runs.
pub fn step_for(command: &Command) -> Option<Step> {
	match command {
		Command::Rename { source, name } => {
			let from = local(source)?;
			Some(Step::Renamed {
				to: from.parent()?.join(std::ffi::OsStr::from_bytes(name.as_bytes())),
				from,
			})
		}
		Command::CreateFolder { parent, name } | Command::CreateFile { parent, name, .. } => {
			Some(Step::Made(vec![local(parent)?.join(std::ffi::OsStr::from_bytes(name.as_bytes()))]))
		}
		Command::Copy { sources, destination, .. } | Command::Link { sources, destination } => {
			let folder = local(destination)?;
			let made: Vec<PathBuf> = sources
				.iter()
				.filter_map(local)
				.filter_map(|source| Some(folder.join(source.file_name()?)))
				.filter(|target| std::fs::symlink_metadata(target).is_err())
				.collect();
			(made.len() == sources.len()).then_some(Step::Made(made))
		}
		Command::Move { sources, destination, .. } => {
			let folder = local(destination)?;
			let moved: Vec<(PathBuf, PathBuf)> = sources
				.iter()
				.filter_map(local)
				.filter_map(|source| {
					let target = folder.join(source.file_name()?);
					std::fs::symlink_metadata(&target).is_err().then_some((source, target))
				})
				.collect();
			(moved.len() == sources.len()).then_some(Step::Moved(moved))
		}
		Command::Trash { targets } => {
			let originals: Vec<PathBuf> = targets.iter().filter_map(local).collect();
			(originals.len() == targets.len()).then(|| Step::Trashed { originals, since: now() - 1 })
		}
		_ => None,
	}
}

/// What undoing `step` takes.
pub fn undo(step: Step) -> Undo {
	match step {
		Step::Renamed { from, to } => {
			let name = from.file_name().map(|name| NativePath::new(name.as_bytes().to_vec()));
			Undo::Run(name.map(|name| Command::Rename { source: at(&to), name }).into_iter().collect())
		}
		Step::Made(made) => {
			let targets: Vec<Location> = made
				.iter()
				.filter(|path| std::fs::symlink_metadata(path).is_ok())
				.map(|path| at(path))
				.collect();
			Undo::Run(if targets.is_empty() { Vec::new() } else { vec![Command::Trash { targets }] })
		}
		Step::Moved(moved) => {
			// One move per original folder, back to where each came from.
			let mut by_folder: Vec<(PathBuf, Vec<Location>)> = Vec::new();
			for (from, now) in moved {
				let Some(folder) = from.parent().map(PathBuf::from) else { continue };
				match by_folder.iter_mut().find(|(known, _)| *known == folder) {
					Some((_, sources)) => sources.push(at(&now)),
					None => by_folder.push((folder, vec![at(&now)])),
				}
			}
			Undo::Run(
				by_folder
					.into_iter()
					.map(|(folder, sources)| Command::Move {
						sources,
						destination: at(&folder),
						conflict_policy: ConflictPolicy::Abort,
					})
					.collect(),
			)
		}
		Step::Trashed { originals, since } => Undo::Restore { originals, since },
		Step::RenamedMany(pairs) => Undo::RenameBack(pairs),
	}
}

fn local(location: &Location) -> Option<PathBuf> {
	(location.backend == BackendId::LOCAL)
		.then(|| PathBuf::from(std::ffi::OsStr::from_bytes(location.path.as_bytes())))
}

fn at(path: &std::path::Path) -> Location { Location::local(NativePath::from(path)) }

fn now() -> i64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(0))
}

#[cfg(test)]
mod tests {
	use std::fs;

	use thallium_actions::Command;
	use thallium_fs_core::{ConflictPolicy, Location, NativePath};

	use super::{Step, Undo, step_for, undo};

	fn at(path: &std::path::Path) -> Location { Location::local(NativePath::from(path)) }

	#[test]
	fn a_rename_undoes_to_the_old_name() {
		let step = step_for(&Command::Rename {
			source: at("/f/old.txt".as_ref()),
			name:   NativePath::new(b"new.txt".to_vec()),
		})
		.expect("step");
		assert_eq!(step, Step::Renamed { from: "/f/old.txt".into(), to: "/f/new.txt".into() });
		let Undo::Run(commands) = undo(step) else { panic!("a rename undoes by running") };
		assert_eq!(commands, [Command::Rename {
			source: at("/f/new.txt".as_ref()),
			name:   NativePath::new(b"old.txt".to_vec()),
		}]);
	}

	#[test]
	fn a_copy_onto_a_taken_name_is_not_recorded() {
		let root = tempfile::tempdir().expect("root");
		fs::write(root.path().join("a.txt"), b"").expect("seed");
		fs::create_dir(root.path().join("to")).expect("target");
		let copy = |destination: &std::path::Path| Command::Copy {
			sources:         vec![at(&root.path().join("a.txt"))],
			destination:     at(destination),
			conflict_policy: ConflictPolicy::Rename,
		};
		assert_eq!(
			step_for(&copy(&root.path().join("to"))),
			Some(Step::Made(vec![root.path().join("to/a.txt")]))
		);
		assert_eq!(
			step_for(&copy(root.path())),
			None,
			"the copy would be renamed; undo cannot know to what"
		);
	}

	#[test]
	fn moves_go_back_to_each_original_folder() {
		let step = Step::Moved(vec![
			("/a/one".into(), "/z/one".into()),
			("/b/two".into(), "/z/two".into()),
			("/a/three".into(), "/z/three".into()),
		]);
		let Undo::Run(commands) = undo(step) else { panic!("a move undoes by running") };
		assert_eq!(commands.len(), 2, "one move per original folder");
		assert!(matches!(&commands[0], Command::Move { sources, destination, .. }
			if sources.len() == 2 && *destination == at("/a".as_ref())));
	}
}
