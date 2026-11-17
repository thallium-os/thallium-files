//! Extracting and creating archives.
//!
//! Zip is handled in-process, since it is the format most often met and the
//! one least often installed as a tool. Tar formats go to the system `tar`,
//! which picks the compressor from the name; anything else goes to `bsdtar`
//! when it is installed. Jobs run on a thread of their own.

use std::{fs, io, os::unix::fs::PermissionsExt, path::{Path, PathBuf}, process::Command, sync::{Arc, Mutex}, thread};

use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Formats a new archive can be written in, by the suffix that names them.
pub const FORMATS: &[&str] = &[".zip", ".tar.gz", ".tar.xz", ".tar.zst"];

/// Suffixes recognised as archives, longest first so `.tar.gz` wins over `.gz`.
const SUFFIXES: &[&str] = &[
	".tar.gz",
	".tar.xz",
	".tar.zst",
	".tar.bz2",
	".tar.lz",
	".tar.lzma",
	".tgz",
	".txz",
	".tbz2",
	".tzst",
	".zip",
	".tar",
	".7z",
	".rar",
	".iso",
	".cpio",
	".xz",
	".gz",
	".bz2",
	".zst",
];

/// The archive suffix of `name`, if it has one.
pub fn suffix(name: &str) -> Option<&'static str> {
	let lower = name.to_lowercase();
	SUFFIXES.iter().copied().find(|suffix| lower.ends_with(suffix) && lower.len() > suffix.len())
}

/// A job in flight. `None` while it runs, then where its result landed, or
/// why it failed.
#[derive(Clone, Default)]
pub struct Job(Arc<Mutex<Option<Result<PathBuf, String>>>>);

impl Job {
	fn spawn(work: impl FnOnce() -> Result<PathBuf, String> + Send + 'static) -> Self {
		let job = Self::default();
		let slot = Arc::clone(&job.0);
		thread::spawn(move || {
			let outcome = work();
			if let Ok(mut slot) = slot.lock() {
				*slot = Some(outcome);
			}
		});
		job
	}

	pub fn take(&self) -> Option<Result<PathBuf, String>> { self.0.lock().ok()?.take() }
}

/// Extract `archive` beside itself. One top-level entry lands as it is;
/// several are gathered in a folder named after the archive, so an archive
/// never scatters files across the folder it is opened in.
pub fn extract(archive: PathBuf) -> Job {
	Job::spawn(move || {
		let folder = archive.parent().ok_or("the archive has no folder")?.to_path_buf();
		let name =
			archive.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
		let stem =
			suffix(&name).map_or(name.as_str(), |suffix| &name[..name.len() - suffix.len()]).to_owned();
		let staging = folder.join(format!(".{stem}.extracting-{}", std::process::id()));
		fs::create_dir(&staging).map_err(|error| error.to_string())?;
		let unpacked = unpack(&archive, &staging).and_then(|()| settle(&staging, &folder, &stem));
		let _ = fs::remove_dir_all(&staging);
		unpacked
	})
}

fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
	let name =
		archive.file_name().map(|name| name.to_string_lossy().to_lowercase()).unwrap_or_default();
	if name.ends_with(".zip") {
		let file = fs::File::open(archive).map_err(|error| error.to_string())?;
		// `extract` refuses entries that would land outside `into`.
		return ZipArchive::new(file)
			.and_then(|mut zip| zip.extract(into))
			.map_err(|error| error.to_string());
	}
	let tar_family =
		name.contains(".tar") || [".tgz", ".txz", ".tbz2", ".tzst"].iter().any(|s| name.ends_with(s));
	let tool = if on_path("bsdtar") {
		"bsdtar"
	} else if tar_family {
		"tar"
	} else {
		""
	};
	if tool.is_empty() {
		return Err("Extracting this kind of archive needs bsdtar (libarchive-tools)".to_owned());
	}
	run(Command::new(tool).arg("-xf").arg(archive).arg("-C").arg(into))
}

/// Move what was unpacked into `staging` out to `folder`, returning where it
/// went.
fn settle(staging: &Path, folder: &Path, stem: &str) -> Result<PathBuf, String> {
	let entries: Vec<PathBuf> = fs::read_dir(staging)
		.map_err(|error| error.to_string())?
		.filter_map(Result::ok)
		.map(|entry| entry.path())
		.collect();
	if let [only] = entries.as_slice() {
		let name = only.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
		let target = free_path(folder, &name);
		fs::rename(only, &target).map_err(|error| error.to_string())?;
		return Ok(target);
	}
	let target = free_path(folder, stem);
	fs::create_dir(&target).map_err(|error| error.to_string())?;
	for entry in entries {
		if let Some(name) = entry.file_name() {
			fs::rename(&entry, target.join(name)).map_err(|error| error.to_string())?;
		}
	}
	Ok(target)
}

/// Pack `sources`, which share one folder, into `output`. The format comes
/// from the output's suffix, one of [`FORMATS`].
pub fn compress(sources: Vec<PathBuf>, output: PathBuf) -> Job {
	Job::spawn(move || {
		let base =
			sources.first().and_then(|first| first.parent()).ok_or("nothing to compress")?.to_path_buf();
		if output.exists() {
			return Err(format!("{} already exists", output.display()));
		}
		let name = output.to_string_lossy().to_lowercase();
		if !FORMATS.iter().any(|format| name.ends_with(format)) {
			return Err(format!("Archives can be written as {}", FORMATS.join(", ")));
		}
		let written = if name.ends_with(".zip") {
			write_zip(&base, &sources, &output).map_err(|error| error.to_string())
		} else {
			let mut command = Command::new("tar");
			command.arg("-caf").arg(&output).arg("-C").arg(&base).arg("--");
			for source in &sources {
				command.arg(source.file_name().unwrap_or_default());
			}
			run(&mut command)
		};
		if written.is_err() {
			let _ = fs::remove_file(&output);
		}
		written.map(|()| output)
	})
}

fn write_zip(base: &Path, sources: &[PathBuf], output: &Path) -> io::Result<()> {
	let mut zip = ZipWriter::new(fs::File::create_new(output)?);
	let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
	let mut pending: Vec<PathBuf> = sources.to_vec();
	while let Some(path) = pending.pop() {
		let relative =
			path.strip_prefix(base).map_err(io::Error::other)?.to_string_lossy().into_owned();
		let metadata = fs::symlink_metadata(&path)?;
		let options = options.unix_permissions(metadata.permissions().mode());
		if metadata.file_type().is_symlink() {
			zip
				.add_symlink(relative, fs::read_link(&path)?.to_string_lossy(), options)
				.map_err(io::Error::other)?;
		} else if metadata.is_dir() {
			zip.add_directory(format!("{relative}/"), options).map_err(io::Error::other)?;
			for entry in fs::read_dir(&path)? {
				pending.push(entry?.path());
			}
		} else {
			zip.start_file(relative, options).map_err(io::Error::other)?;
			io::copy(&mut fs::File::open(&path)?, &mut zip)?;
		}
	}
	zip.finish().map_err(io::Error::other)?;
	Ok(())
}

fn run(command: &mut Command) -> Result<(), String> {
	let output = command.output().map_err(|error| error.to_string())?;
	if output.status.success() {
		Ok(())
	} else {
		Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
	}
}

fn on_path(name: &str) -> bool {
	std::env::var_os("PATH")
		.is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

/// `folder/name`, or `folder/name 2`, ... whichever does not exist yet.
fn free_path(folder: &Path, name: &str) -> PathBuf {
	let first = folder.join(name);
	if fs::symlink_metadata(&first).is_err() {
		return first;
	}
	(2..)
		.map(|number| folder.join(format!("{name} {number}")))
		.find(|path| fs::symlink_metadata(path).is_err())
		.unwrap_or(first)
}

#[cfg(test)]
mod tests {
	use std::{fs, path::Path, time::Duration};

	use super::{Job, compress, extract, suffix};

	fn wait(job: &Job) -> Result<std::path::PathBuf, String> {
		loop {
			if let Some(outcome) = job.take() {
				return outcome;
			}
			std::thread::sleep(Duration::from_millis(5));
		}
	}

	fn seed(root: &Path) {
		fs::create_dir_all(root.join("project/src")).expect("tree");
		fs::write(root.join("project/src/main.rs"), b"fn main() {}").expect("file");
		fs::write(root.join("notes.txt"), b"notes").expect("file");
	}

	#[test]
	fn suffixes_prefer_the_longest_match() {
		assert_eq!(suffix("a.tar.gz"), Some(".tar.gz"));
		assert_eq!(suffix("A.ZIP"), Some(".zip"));
		assert_eq!(suffix("plain.txt"), None);
		assert_eq!(suffix(".zip"), None, "a bare suffix is a hidden file, not an archive");
	}

	#[test]
	fn zip_and_tar_round_trip_into_a_folder_named_after_the_archive() {
		for format in [".zip", ".tar.gz", ".tar.xz"] {
			let root = tempfile::tempdir().expect("root");
			seed(root.path());
			let output = root.path().join(format!("bundle{format}"));
			let sources = vec![root.path().join("project"), root.path().join("notes.txt")];
			assert_eq!(wait(&compress(sources, output.clone())).expect("compress"), output);

			let landed = wait(&extract(output)).expect("extract");
			assert_eq!(landed, root.path().join("bundle"), "{format}: two entries gather in a folder");
			assert_eq!(fs::read(landed.join("project/src/main.rs")).expect("deep file"), b"fn main() {}");
			assert_eq!(fs::read(landed.join("notes.txt")).expect("top file"), b"notes");
			assert!(
				fs::read_dir(root.path()).expect("list").all(|entry| {
					!entry.expect("entry").file_name().to_string_lossy().contains(".extracting-")
				}),
				"{format}: staging is cleaned up"
			);
		}
	}

	#[test]
	fn unknown_formats_are_refused_rather_than_mislabelled() {
		let root = tempfile::tempdir().expect("root");
		seed(root.path());
		let output = root.path().join("bundle.7z");
		assert!(wait(&compress(vec![root.path().join("notes.txt")], output.clone())).is_err());
		assert!(!output.exists());
	}

	#[test]
	fn a_single_top_level_folder_lands_as_itself_beside_a_clash() {
		let root = tempfile::tempdir().expect("root");
		seed(root.path());
		let output = root.path().join("only.zip");
		wait(&compress(vec![root.path().join("project")], output.clone())).expect("compress");
		let landed = wait(&extract(output)).expect("extract");
		assert_eq!(landed, root.path().join("project 2"), "the existing folder is not merged into");
		assert!(landed.join("src/main.rs").is_file());
	}
}
