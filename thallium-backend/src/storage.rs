//! What a directory holds, and how much room the disk it sits on has.
//!
//! Two different questions with two different costs. The disk comes from the
//! kernel with `statvfs` and is free. What a folder holds can only be counted,
//! and counting means a stat per file, so it runs on workers and reports as it
//! goes rather than making the window wait for a total.

use std::{collections::{HashMap, VecDeque}, ffi::CString, fs, os::unix::ffi::OsStrExt, path::{Path, PathBuf}, sync::{Arc, Condvar, Mutex, atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}}, thread, time::{Duration, Instant}};

/// How long a finished count stands before the tree is walked again.
///
/// Counting a large tree is a stat per file, so doing it afresh on every
/// navigation would make stepping in and out of a folder one of the most
/// expensive things the window does. Sizes do drift, hence a limit rather than
/// keeping the answer for the whole session.
const COUNT_TTL: Duration = Duration::from_secs(300);

/// Capacity of one mounted filesystem, in bytes.
#[derive(Clone, Copy, Default)]
pub struct Usage {
	pub total: u64,
	/// Free space as seen by this user, which is what they can actually fill.
	pub free:  u64,
}

impl Usage {
	#[must_use]
	pub fn used(&self) -> u64 { self.total.saturating_sub(self.free) }

	/// Used space as a fraction, or zero when the size is not known.
	#[must_use]
	pub fn share_used(&self) -> f64 {
		if self.total == 0 {
			return 0.0;
		}
		self.used() as f64 / self.total as f64
	}
}

/// Read the usage of the filesystem `path` lives on.
///
/// Returns `None` when the path cannot be queried, which is the honest answer
/// for a remote: its capacity is not this machine's to report.
#[must_use]
pub fn of(path: &Path) -> Option<Usage> {
	let raw = CString::new(path.as_os_str().as_bytes()).ok()?;
	// SAFETY: the pointer is a valid NUL-terminated path for the duration of
	// the call, and the struct is only read after statvfs reports success.
	let stats = unsafe {
		let mut stats: libc::statvfs = core::mem::zeroed();
		if libc::statvfs(raw.as_ptr(), &raw mut stats) != 0 {
			return None;
		}
		stats
	};
	// `f_frsize` is the fragment size the block counts are expressed in;
	// `f_bsize` is the preferred I/O size and is not always the same number.
	let block = if stats.f_frsize > 0 { stats.f_frsize } else { stats.f_bsize } as u64;
	Some(Usage { total: stats.f_blocks as u64 * block, free: stats.f_bavail as u64 * block })
}

#[derive(Default)]
struct Queue {
	pending: VecDeque<PathBuf>,
	busy:    usize,
}

/// One count's state. Every count gets a fresh one, so workers still
/// finishing an abandoned count only ever touch their own: sharing one let
/// a stale worker decrement the next count's queue below zero.
#[derive(Default)]
struct Counting {
	queue:     Mutex<Queue>,
	wake:      Condvar,
	bytes:     AtomicU64,
	files:     AtomicU64,
	walking:   AtomicUsize,
	/// Set when the count is abandoned; its workers stop at the next folder.
	cancelled: AtomicBool,
}

/// What a finished count found.
#[derive(Clone, Copy)]
struct Counted {
	bytes: u64,
	files: u64,
	at:    Instant,
}

/// A running count of what one directory holds.
pub struct Measure {
	/// The count on screen; replaced, never reset, by each new one.
	current: Mutex<Arc<Counting>>,
	/// Directories already counted, so returning to one is free.
	known:   Arc<Mutex<HashMap<PathBuf, Counted>>>,
	workers: usize,
}

impl Measure {
	#[must_use]
	pub fn new() -> Self {
		let workers = thread::available_parallelism().map_or(4, std::num::NonZero::get);
		Self {
			current: Mutex::new(Arc::default()),
			known:   Arc::default(),
			workers: workers.clamp(1, 8),
		}
	}

	/// Abandon any count in progress and report what `root` holds.
	///
	/// A directory counted recently is answered from memory and no walk is
	/// started at all; otherwise the walk runs and its total is remembered.
	pub fn start(&self, root: PathBuf) {
		self.cancel();
		let shared = Arc::new(Counting::default());
		if let Some(known) = self.remembered(&root) {
			shared.bytes.store(known.bytes, Ordering::Relaxed);
			shared.files.store(known.files, Ordering::Relaxed);
			self.replace(shared);
			return;
		}

		if let Ok(mut queue) = shared.queue.lock() {
			queue.pending.push_back(root.clone());
		}
		shared.walking.store(self.workers, Ordering::SeqCst);
		self.replace(Arc::clone(&shared));
		for _ in 0..self.workers {
			let shared = Arc::clone(&shared);
			let known = Arc::clone(&self.known);
			let root = root.clone();
			thread::spawn(move || {
				count(&shared);
				// The worker that turns the last light out records the total,
				// and only if this count was never abandoned: a partial sum
				// remembered as final would be wrong for as long as it stood.
				let remaining = shared.walking.fetch_sub(1, Ordering::SeqCst);
				if remaining == 1
					&& !shared.cancelled.load(Ordering::SeqCst)
					&& let Ok(mut known) = known.lock()
				{
					known.insert(root, Counted {
						bytes: shared.bytes.load(Ordering::Relaxed),
						files: shared.files.load(Ordering::Relaxed),
						at:    Instant::now(),
					});
				}
			});
		}
	}

	fn remembered(&self, root: &Path) -> Option<Counted> {
		let known = self.known.lock().ok()?;
		known.get(root).copied().filter(|counted| counted.at.elapsed() < COUNT_TTL)
	}

	/// Forget what a directory held, so the next visit counts it again.
	pub fn forget(&self, root: &Path) {
		if let Ok(mut known) = self.known.lock() {
			known.remove(root);
		}
	}

	pub fn cancel(&self) {
		let shared = self.current();
		shared.cancelled.store(true, Ordering::SeqCst);
		if let Ok(mut queue) = shared.queue.lock() {
			queue.pending.clear();
		}
		shared.wake.notify_all();
	}

	#[must_use]
	pub fn walking(&self) -> bool { self.current().walking.load(Ordering::SeqCst) > 0 }

	/// Bytes counted so far, which is the total once the walk has finished.
	#[must_use]
	pub fn bytes(&self) -> u64 { self.current().bytes.load(Ordering::Relaxed) }

	#[must_use]
	pub fn files(&self) -> u64 { self.current().files.load(Ordering::Relaxed) }

	fn current(&self) -> Arc<Counting> {
		self.current.lock().map(|current| Arc::clone(&current)).unwrap_or_default()
	}

	fn replace(&self, shared: Arc<Counting>) {
		if let Ok(mut current) = self.current.lock() {
			*current = shared;
		}
	}
}

impl Default for Measure {
	fn default() -> Self { Self::new() }
}

fn count(shared: &Arc<Counting>) {
	loop {
		let directory = {
			let Ok(mut queue) = shared.queue.lock() else { return };
			loop {
				if shared.cancelled.load(Ordering::SeqCst) {
					return;
				}
				if let Some(next) = queue.pending.pop_front() {
					queue.busy += 1;
					break next;
				}
				if queue.busy == 0 {
					shared.wake.notify_all();
					return;
				}
				let Ok(waited) = shared.wake.wait(queue) else { return };
				queue = waited;
			}
		};

		let mut children = Vec::new();
		let mut bytes = 0u64;
		let mut files = 0u64;
		if let Ok(entries) = fs::read_dir(&directory) {
			for entry in entries.flatten() {
				let Ok(kind) = entry.file_type() else { continue };
				// Symbolic links are not followed: their target is counted
				// where it actually lives, and a loop would never end.
				if kind.is_symlink() {
					continue;
				}
				if kind.is_dir() {
					children.push(entry.path());
				} else if let Ok(facts) = entry.metadata() {
					bytes += facts.len();
					files += 1;
				}
			}
		}
		shared.bytes.fetch_add(bytes, Ordering::Relaxed);
		shared.files.fetch_add(files, Ordering::Relaxed);

		let Ok(mut queue) = shared.queue.lock() else { return };
		queue.pending.extend(children);
		queue.busy -= 1;
		shared.wake.notify_all();
	}
}

#[cfg(test)]
mod tests {
	use std::{fs, path::Path, thread, time::Duration};

	use super::{Measure, Usage, of};

	#[test]
	fn restarting_mid_count_neither_panics_nor_mixes_totals() {
		let root = std::env::temp_dir().join(format!("thallium-restart-{}", std::process::id()));
		for folder in 0..40 {
			let inner = root.join(format!("big/{folder}/a/b"));
			fs::create_dir_all(&inner).expect("tree");
			fs::write(inner.join("f"), vec![0u8; 10]).expect("file");
		}
		fs::create_dir_all(root.join("small")).expect("small");
		fs::write(root.join("small/one"), vec![0u8; 7]).expect("file");

		let measure = Measure::new();
		for _ in 0..50 {
			measure.start(root.join("big"));
			measure.start(root.join("small"));
		}
		for _ in 0..400 {
			if !measure.walking() {
				break;
			}
			thread::sleep(Duration::from_millis(5));
		}
		assert!(!measure.walking(), "the last count finishes");
		assert_eq!((measure.bytes(), measure.files()), (7, 1), "abandoned counts add nothing");
		let _ = fs::remove_dir_all(root);
	}

	#[test]
	fn a_folder_is_counted_through_its_subdirectories() {
		let root = std::env::temp_dir().join(format!("thallium-measure-{}", std::process::id()));
		fs::create_dir_all(root.join("inner")).expect("tree");
		fs::write(root.join("a.bin"), vec![0u8; 1000]).expect("file");
		fs::write(root.join("inner/b.bin"), vec![0u8; 2000]).expect("file");

		let measure = Measure::new();
		measure.start(root.clone());
		for _ in 0..400 {
			if !measure.walking() {
				break;
			}
			thread::sleep(Duration::from_millis(5));
		}
		assert!(!measure.walking(), "count did not finish");
		assert_eq!(measure.bytes(), 3000);
		assert_eq!(measure.files(), 2);

		let _ = fs::remove_dir_all(&root);
	}

	#[test]
	fn a_counted_folder_is_answered_from_memory_the_next_time() {
		let root = std::env::temp_dir().join(format!("thallium-cached-{}", std::process::id()));
		fs::create_dir_all(&root).expect("tree");
		fs::write(root.join("a.bin"), vec![0u8; 512]).expect("file");

		let measure = Measure::new();
		measure.start(root.clone());
		for _ in 0..400 {
			if !measure.walking() {
				break;
			}
			thread::sleep(Duration::from_millis(5));
		}
		assert_eq!(measure.bytes(), 512);

		// The second ask must not walk at all.
		measure.start(root.clone());
		assert!(!measure.walking(), "a remembered folder starts no walk");
		assert_eq!(measure.bytes(), 512);

		// Until it is forgotten.
		measure.forget(&root);
		measure.start(root.clone());
		for _ in 0..400 {
			if !measure.walking() {
				break;
			}
			thread::sleep(Duration::from_millis(5));
		}
		assert_eq!(measure.bytes(), 512);

		let _ = fs::remove_dir_all(&root);
	}

	#[test]
	fn a_cancelled_count_reports_nothing_new() {
		let measure = Measure::new();
		measure.start(std::env::temp_dir());
		measure.cancel();
		for _ in 0..400 {
			if !measure.walking() {
				break;
			}
			thread::sleep(Duration::from_millis(5));
		}
		assert!(!measure.walking());
	}

	#[test]
	fn a_real_directory_reports_a_plausible_size() {
		let usage = of(Path::new("/")).expect("root is mounted");
		assert!(usage.total > 0, "a mounted filesystem has a size");
		assert!(usage.free <= usage.total);
		assert!((0.0..=1.0).contains(&usage.share_used()));
	}

	#[test]
	fn a_path_that_does_not_exist_reports_nothing() {
		assert!(of(Path::new("/nonexistent/place/at/all")).is_none());
	}

	#[test]
	fn an_unknown_size_is_not_divided_by() {
		assert_eq!(Usage::default().share_used(), 0.0);
		assert_eq!(Usage::default().used(), 0);
	}
}
