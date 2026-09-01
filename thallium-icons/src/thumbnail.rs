//! Thumbnails, stored where the rest of the desktop already looks for them.
//!
//! This follows the freedesktop thumbnail managing standard: the file name is
//! the MD5 of the source URI, and the PNG carries `Thumb::URI` and
//! `Thumb::MTime` so a stale thumbnail is recognised rather than shown. Other
//! applications' thumbnails are therefore reused, and the ones written here
//! are reused by them.

use std::{collections::{HashMap, HashSet}, fs::{self, File}, io::BufWriter, os::unix::{ffi::OsStrExt, fs::PermissionsExt}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}, mpsc::{self, Receiver, Sender}}, thread};

use md5::{Digest, Md5};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_encode};

/// The `normal` bucket of the specification. The browser draws icons at 24px
/// and tiles at 52px, so the 256px `large` bucket would be paid for and thrown
/// away.
const THUMBNAIL_SIZE: u32 = 128;
/// Workers. Decoding is CPU-bound, and a directory of photographs should not
/// arrive one image at a time.
const WORKERS: usize = 2;
/// Characters `g_filename_to_uri` leaves unescaped. Matching it means this
/// store and every GLib-based thumbnailer agree on the hash of a path.
const URI_UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
	.remove(b'-')
	.remove(b'_')
	.remove(b'.')
	.remove(b'~')
	.remove(b'!')
	.remove(b'$')
	.remove(b'&')
	.remove(b'\'')
	.remove(b'(')
	.remove(b')')
	.remove(b'*')
	.remove(b'+')
	.remove(b',')
	.remove(b';')
	.remove(b'=')
	.remove(b':')
	.remove(b'@')
	.remove(b'/');

/// What the frontend should draw for one entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Thumbnail {
	/// A thumbnail exists on disk and is current.
	Ready(PathBuf),
	/// Being generated. The caller should draw the type icon meanwhile.
	Pending,
	/// Nothing will arrive: the type has no thumbnailer, or generation failed.
	Unavailable,
}

struct Job {
	source: PathBuf,
	target: PathBuf,
	uri:    String,
	mtime:  i64,
	video:  bool,
}

/// What is known about each source path. Held together so a repaint answers
/// from memory: the view asks for every visible row's thumbnail on every
/// revision, and probing the cache directory each time would put a file read
/// behind every frame.
#[derive(Default)]
struct Known {
	/// Source path to the modification time the thumbnail was made for and the
	/// file holding it.
	ready:  HashMap<PathBuf, (i64, PathBuf)>,
	queued: HashSet<PathBuf>,
	/// Sources with no usable thumbnail, and the mtime that failed, so a file
	/// that is later replaced is tried again.
	failed: HashMap<PathBuf, i64>,
}

pub struct ThumbnailStore {
	/// `.../thumbnails/normal`, absent when there is no usable cache directory.
	directory: Option<PathBuf>,
	/// `.../thumbnails/large`. Only read: other applications write 256px
	/// thumbnails there, and reusing one beats generating a smaller copy.
	large:     Option<PathBuf>,
	known:     Arc<Mutex<Known>>,
	completed: Arc<AtomicUsize>,
	sender:    Option<Sender<Job>>,
	/// External helper for video frames, when one is installed.
	video:     Option<PathBuf>,
}

impl ThumbnailStore {
	pub fn new() -> Self {
		let directory = cache_directory("normal");
		let large = cache_directory("large");
		let video = which("ffmpegthumbnailer");
		let known = Arc::new(Mutex::new(Known::default()));
		let completed = Arc::new(AtomicUsize::new(0));

		let sender = directory.as_ref().map(|_| {
			let (sender, receiver) = mpsc::channel::<Job>();
			let receiver = Arc::new(Mutex::new(receiver));
			for _ in 0..WORKERS {
				let receiver = Arc::clone(&receiver);
				let known = Arc::clone(&known);
				let completed = Arc::clone(&completed);
				let video = video.clone();
				thread::spawn(move || worker(&receiver, &known, &completed, video.as_deref()));
			}
			sender
		});

		Self { directory, large, known, completed, sender, video }
	}

	/// Whether this store can produce anything for `mime` at all.
	pub fn handles(&self, mime: &str) -> bool {
		mime.starts_with("image/") || (self.video.is_some() && mime.starts_with("video/"))
	}

	/// The thumbnail for `source`, queueing generation when there is none.
	///
	/// Answers from memory once a path is known, so the view can ask on every
	/// repaint without touching the disk.
	pub fn request(&self, source: &Path, mime: &str, mtime: i64) -> Thumbnail {
		let (Some(directory), Some(sender)) = (self.directory.as_ref(), self.sender.as_ref()) else {
			return Thumbnail::Unavailable;
		};
		if !self.handles(mime) {
			return Thumbnail::Unavailable;
		}
		let Ok(mut known) = self.known.lock() else {
			return Thumbnail::Unavailable;
		};

		// A recorded answer only holds while the source has not been written to
		// since; otherwise it is dropped and the path is reconsidered.
		if let Some((generation, file)) = known.ready.get(source).cloned() {
			if generation == mtime {
				return Thumbnail::Ready(file);
			}
			known.ready.remove(source);
		}
		if let Some(generation) = known.failed.get(source).copied() {
			if generation == mtime {
				return Thumbnail::Unavailable;
			}
			known.failed.remove(source);
		}
		if known.queued.contains(source) {
			return Thumbnail::Pending;
		}

		// First sight of this path: look in the shared cache once. Another
		// application may already have made the thumbnail this session needs.
		let uri = file_uri(source);
		let name = format!("{}.png", hash_of(&uri));
		let existing = self
			.large
			.iter()
			.chain(self.directory.iter())
			.map(|bucket| bucket.join(&name))
			.find(|candidate| is_current(candidate, mtime));
		if let Some(existing) = existing {
			known.ready.insert(source.to_path_buf(), (mtime, existing.clone()));
			return Thumbnail::Ready(existing);
		}

		known.queued.insert(source.to_path_buf());
		drop(known);

		let job = Job {
			source: source.to_path_buf(),
			target: directory.join(&name),
			uri,
			mtime,
			video: mime.starts_with("video/"),
		};
		if sender.send(job).is_err() {
			return Thumbnail::Unavailable;
		}
		Thumbnail::Pending
	}

	/// How many jobs have finished since the last call. A non-zero answer means
	/// the frontend has new thumbnails to pick up.
	pub fn take_completed(&self) -> usize { self.completed.swap(0, Ordering::AcqRel) }
}

impl Default for ThumbnailStore {
	fn default() -> Self { Self::new() }
}

fn worker(
	receiver: &Mutex<Receiver<Job>>,
	known: &Mutex<Known>,
	completed: &AtomicUsize,
	video: Option<&Path>,
) {
	loop {
		// The lock is held only for the receive, so workers do not serialise on
		// each other while generating.
		let job = {
			let Ok(receiver) = receiver.lock() else { return };
			match receiver.recv() {
				Ok(job) => job,
				Err(_) => return,
			}
		};

		let produced = generate(&job, video).is_ok();
		if let Ok(mut known) = known.lock() {
			known.queued.remove(&job.source);
			if produced {
				known.ready.insert(job.source.clone(), (job.mtime, job.target.clone()));
			} else {
				known.failed.insert(job.source.clone(), job.mtime);
			}
		}
		completed.fetch_add(1, Ordering::AcqRel);
	}
}

fn generate(job: &Job, video: Option<&Path>) -> Result<(), ()> {
	if job.video {
		let helper = video.ok_or(())?;
		let status = Command::new(helper)
			.arg("-i")
			.arg(&job.source)
			.arg("-o")
			.arg(&job.target)
			.arg("-s")
			.arg(THUMBNAIL_SIZE.to_string())
			.stdout(Stdio::null())
			.stderr(Stdio::null())
			.status()
			.map_err(|_| ())?;
		return if status.success() && job.target.exists() { Ok(()) } else { Err(()) };
	}

	let image = image::ImageReader::open(&job.source)
		.map_err(|_| ())?
		.with_guessed_format()
		.map_err(|_| ())?
		.decode()
		.map_err(|_| ())?
		.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE)
		.into_rgba8();

	write_png(&job.target, &image, &job.uri, job.mtime)
}

/// Written beside the target and renamed, so a reader never sees a half-written
/// thumbnail and a crashed worker leaves no corrupt cache entry.
fn write_png(target: &Path, image: &image::RgbaImage, uri: &str, mtime: i64) -> Result<(), ()> {
	let staging = target.with_extension(format!("tmp{}", std::process::id()));
	{
		let file = File::create(&staging).map_err(|_| ())?;
		let _ = file.set_permissions(fs::Permissions::from_mode(0o600));
		let mut encoder = png::Encoder::new(BufWriter::new(file), image.width(), image.height());
		encoder.set_color(png::ColorType::Rgba);
		encoder.set_depth(png::BitDepth::Eight);
		// The two keys the specification requires for staleness checks.
		encoder.add_text_chunk("Thumb::URI".to_owned(), uri.to_owned()).map_err(|_| ())?;
		encoder.add_text_chunk("Thumb::MTime".to_owned(), mtime.to_string()).map_err(|_| ())?;
		let mut writer = encoder.write_header().map_err(|_| ())?;
		writer.write_image_data(image.as_raw()).map_err(|_| ())?;
	}
	fs::rename(&staging, target).map_err(|_| {
		let _ = fs::remove_file(&staging);
	})
}

/// A cached thumbnail counts only when its recorded mtime still matches the
/// source; otherwise the file has changed underneath it.
fn is_current(target: &Path, mtime: i64) -> bool {
	let Ok(file) = File::open(target) else { return false };
	let Ok(reader) = png::Decoder::new(file).read_info() else { return false };
	let info = reader.info();
	let recorded = info
		.uncompressed_latin1_text
		.iter()
		.find(|chunk| chunk.keyword == "Thumb::MTime")
		.map(|chunk| chunk.text.clone())
		.or_else(|| {
			info
				.compressed_latin1_text
				.iter()
				.find(|chunk| chunk.keyword == "Thumb::MTime")
				.and_then(|chunk| chunk.get_text().ok())
		});
	recorded.and_then(|text| text.trim().parse::<i64>().ok()) == Some(mtime)
}

fn cache_directory(bucket: &str) -> Option<PathBuf> {
	let base = std::env::var_os("XDG_CACHE_HOME")
		.filter(|value| !value.is_empty())
		.map(PathBuf::from)
		.or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
	let directory = base.join("thumbnails").join(bucket);
	fs::create_dir_all(&directory).ok()?;
	// The specification asks for 0700; the cache leaks what the user has looked
	// at otherwise.
	let _ = fs::set_permissions(&directory, fs::Permissions::from_mode(0o700));
	Some(directory)
}

pub(crate) fn file_uri(path: &Path) -> String {
	format!("file://{}", percent_encode(path.as_os_str().as_bytes(), URI_UNRESERVED))
}

fn hash_of(uri: &str) -> String {
	let digest = Md5::digest(uri.as_bytes());
	digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn which(program: &str) -> Option<PathBuf> {
	std::env::var_os("PATH").and_then(|paths| {
		std::env::split_paths(&paths)
			.map(|directory| directory.join(program))
			.find(|candidate| candidate.is_file() && is_executable(candidate))
	})
}

fn is_executable(path: &Path) -> bool {
	fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
	use std::path::Path;

	use super::{Thumbnail, ThumbnailStore, file_uri, hash_of};

	#[test]
	fn uris_and_hashes_match_the_specification() {
		// The example given in the thumbnail managing standard.
		assert_eq!(file_uri(Path::new("/home/jens/photo/me.png")), "file:///home/jens/photo/me.png");
		assert_eq!(hash_of("file:///home/jens/photo/me.png"), "d40775e596682f2a16d1b834c221c0a2");
	}

	#[test]
	fn spaces_are_escaped_but_slashes_are_not() {
		assert_eq!(file_uri(Path::new("/tmp/a b/c")), "file:///tmp/a%20b/c");
	}

	#[test]
	fn types_without_a_thumbnailer_are_unavailable() {
		let store = ThumbnailStore::new();
		assert!(!store.handles("text/x-rust"));
		assert_eq!(store.request(Path::new("/tmp/main.rs"), "text/x-rust", 0), Thumbnail::Unavailable);
	}
}
