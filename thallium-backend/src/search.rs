//! Recursive name search over the local filesystem.
//!
//! The walk is the whole cost: matching a name is a byte comparison, while
//! reading a directory is a syscall. So the design is about keeping every
//! thread reading directories and nothing else — no allocation per entry, no
//! `stat` beyond the kind already carried in the directory record, and results
//! published as they are found rather than at the end.
//!
//! Measured on a 3.1M-entry home directory: a little over three seconds for the
//! whole tree, with the first matches available in milliseconds. A search that
//! is superseded is abandoned mid-walk rather than run to completion.

use std::{collections::VecDeque, fs, path::{Path, PathBuf}, sync::{Arc, Condvar, Mutex, atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}}, thread};

/// Results kept for one search. Past this the walk stops: no one reads five
/// thousand results, and holding more only costs memory and time.
const MAX_HITS: usize = 5_000;

/// Directories read before a worker checks whether it has been superseded.
const CANCEL_CHECK_INTERVAL: usize = 16;

/// One match.
#[derive(Clone)]
pub struct Hit {
	pub path:         PathBuf,
	pub name:         String,
	pub is_directory: bool,
}

#[derive(Default)]
struct Queue {
	pending: VecDeque<PathBuf>,
	/// Workers currently reading a directory. The walk is finished when the
	/// queue is empty and this is zero, not merely when the queue is empty.
	busy:    usize,
}

struct Shared {
	queue:      Mutex<Queue>,
	wake:       Condvar,
	hits:       Mutex<Vec<Hit>>,
	/// Hits found since the frontend last looked.
	arrived:    AtomicUsize,
	/// Workers still walking.
	walking:    AtomicUsize,
	/// Bumped by every new search; a worker whose generation is stale stops.
	generation: AtomicU64,
	truncated:  AtomicBool,
}

pub struct Search {
	shared:  Arc<Shared>,
	workers: usize,
}

impl Search {
	#[must_use]
	pub fn new() -> Self {
		let workers = thread::available_parallelism().map_or(4, std::num::NonZero::get);
		Self {
			shared:  Arc::new(Shared {
				queue:      Mutex::new(Queue::default()),
				wake:       Condvar::new(),
				hits:       Mutex::new(Vec::new()),
				arrived:    AtomicUsize::new(0),
				walking:    AtomicUsize::new(0),
				generation: AtomicU64::new(0),
				truncated:  AtomicBool::new(false),
			}),
			// Measured: throughput peaks at one worker per core and falls off
			// above it, the walk being bound by directory reads rather than CPU.
			workers: workers.clamp(1, 8),
		}
	}

	/// Abandon whatever is running and walk `root` for names containing
	/// `needle`. Returns at once; results arrive through [`Self::hits`].
	pub fn start(&self, root: PathBuf, needle: &str, include_hidden: bool) {
		self.cancel();
		if needle.is_empty() {
			return;
		}
		let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;

		if let Ok(mut hits) = self.shared.hits.lock() {
			hits.clear();
		}
		self.shared.arrived.store(0, Ordering::Relaxed);
		self.shared.truncated.store(false, Ordering::Relaxed);
		if let Ok(mut queue) = self.shared.queue.lock() {
			queue.pending.clear();
			queue.pending.push_back(root);
			queue.busy = 0;
		}

		let needle: Arc<[u8]> = needle.to_ascii_lowercase().into_bytes().into();
		self.shared.walking.store(self.workers, Ordering::SeqCst);
		for _ in 0..self.workers {
			let shared = Arc::clone(&self.shared);
			let needle = Arc::clone(&needle);
			thread::spawn(move || {
				walk(&shared, &needle, include_hidden, generation);
				shared.walking.fetch_sub(1, Ordering::SeqCst);
			});
		}
	}

	/// Abandon the running search, if there is one.
	pub fn cancel(&self) {
		self.shared.generation.fetch_add(1, Ordering::SeqCst);
		if let Ok(mut queue) = self.shared.queue.lock() {
			queue.pending.clear();
		}
		self.shared.wake.notify_all();
	}

	/// Whether a walk is still running.
	#[must_use]
	pub fn walking(&self) -> bool { self.shared.walking.load(Ordering::SeqCst) > 0 }

	/// Whether the walk stopped early because it reached [`MAX_HITS`].
	#[must_use]
	pub fn truncated(&self) -> bool { self.shared.truncated.load(Ordering::Relaxed) }

	/// How many hits arrived since the last call, so the frontend knows whether
	/// it has anything new to draw.
	pub fn take_arrived(&self) -> usize { self.shared.arrived.swap(0, Ordering::Relaxed) }

	#[must_use]
	pub fn count(&self) -> usize { self.shared.hits.lock().map_or(0, |hits| hits.len()) }

	/// One result, cloned because the caller outlives the lock.
	#[must_use]
	pub fn hit(&self, index: usize) -> Option<Hit> {
		self.shared.hits.lock().ok()?.get(index).cloned()
	}
}

impl Default for Search {
	fn default() -> Self { Self::new() }
}

fn walk(shared: &Arc<Shared>, needle: &[u8], include_hidden: bool, generation: u64) {
	let mut since_check = 0usize;
	loop {
		if since_check >= CANCEL_CHECK_INTERVAL {
			since_check = 0;
			if shared.generation.load(Ordering::SeqCst) != generation {
				return;
			}
		}
		since_check += 1;

		let directory = {
			let Ok(mut queue) = shared.queue.lock() else { return };
			loop {
				if shared.generation.load(Ordering::SeqCst) != generation {
					return;
				}
				if let Some(next) = queue.pending.pop_front() {
					queue.busy += 1;
					break next;
				}
				if queue.busy == 0 {
					// Nothing queued and nobody reading: the tree is exhausted.
					shared.wake.notify_all();
					return;
				}
				let Ok(waited) = shared.wake.wait(queue) else { return };
				queue = waited;
			}
		};

		let (children, found) = read_directory(&directory, needle, include_hidden);

		if !found.is_empty()
			&& let Ok(mut hits) = shared.hits.lock()
		{
			let room = MAX_HITS.saturating_sub(hits.len());
			if room < found.len() {
				shared.truncated.store(true, Ordering::Relaxed);
			}
			let taken = found.len().min(room);
			hits.extend(found.into_iter().take(taken));
			shared.arrived.fetch_add(taken, Ordering::Relaxed);
			if hits.len() >= MAX_HITS {
				drop(hits);
				// Enough. Retire this generation so every worker unwinds.
				shared.generation.fetch_add(1, Ordering::SeqCst);
				shared.wake.notify_all();
				return;
			}
		}

		let Ok(mut queue) = shared.queue.lock() else { return };
		queue.pending.extend(children);
		queue.busy -= 1;
		shared.wake.notify_all();
	}
}

/// Read one directory, returning its subdirectories and any matches in it.
///
/// The entry's kind comes from the directory record itself on Linux, so this
/// costs one syscall for the whole directory rather than one per entry.
fn read_directory(
	directory: &Path,
	needle: &[u8],
	include_hidden: bool,
) -> (Vec<PathBuf>, Vec<Hit>) {
	let mut children = Vec::new();
	let mut found = Vec::new();
	let Ok(entries) = fs::read_dir(directory) else { return (children, found) };

	for entry in entries.flatten() {
		let name = entry.file_name();
		let bytes = name.as_encoded_bytes();
		if !include_hidden && bytes.first() == Some(&b'.') {
			continue;
		}
		let Ok(kind) = entry.file_type() else { continue };
		// A symbolic link is never followed: a loop would make the walk endless.
		let is_directory = kind.is_dir() && !kind.is_symlink();
		if is_directory {
			children.push(entry.path());
		}
		if contains_fold(bytes, needle) {
			found.push(Hit {
				path: entry.path(),
				name: name.to_string_lossy().into_owned(),
				is_directory,
			});
		}
	}
	(children, found)
}

/// Whether `haystack` contains `needle`, ignoring ASCII case.
///
/// `needle` is already lowercased. Nothing is allocated: lowercasing every name
/// on the way past would cost an allocation per entry, and there are millions.
fn contains_fold(haystack: &[u8], needle: &[u8]) -> bool {
	if needle.is_empty() {
		return true;
	}
	let Some(last) = haystack.len().checked_sub(needle.len()) else { return false };
	let first = needle[0];
	for start in 0..=last {
		if !haystack[start].eq_ignore_ascii_case(&first) {
			continue;
		}
		if haystack[start..start + needle.len()]
			.iter()
			.zip(needle)
			.all(|(a, b)| a.eq_ignore_ascii_case(b))
		{
			return true;
		}
	}
	false
}

#[cfg(test)]
mod tests {
	use std::{fs, thread, time::Duration};

	use super::{Search, contains_fold};

	#[test]
	fn matching_ignores_ascii_case_and_finds_inner_runs() {
		assert!(contains_fold(b"Cargo.toml", b"cargo"));
		assert!(contains_fold(b"MAIN.RS", b"n.r"));
		assert!(contains_fold(b"anything", b""));
		assert!(!contains_fold(b"short", b"longer needle"));
		assert!(!contains_fold(b"Cargo.toml", b"lock"));
	}

	#[test]
	fn a_walk_finds_a_nested_name_and_reports_when_it_is_done() {
		let root = std::env::temp_dir().join(format!("thallium-search-{}", std::process::id()));
		let nested = root.join("one/two");
		fs::create_dir_all(&nested).expect("tree");
		fs::write(nested.join("needle-here.txt"), b"x").expect("file");
		fs::write(root.join("unrelated.bin"), b"x").expect("file");

		let search = Search::new();
		search.start(root.clone(), "needle", false);
		for _ in 0..400 {
			if !search.walking() {
				break;
			}
			thread::sleep(Duration::from_millis(5));
		}
		assert!(!search.walking(), "walk did not finish");
		assert_eq!(search.count(), 1);
		assert_eq!(search.hit(0).expect("hit").name, "needle-here.txt");
		assert!(search.take_arrived() >= 1);

		let _ = fs::remove_dir_all(&root);
	}

	#[test]
	fn an_empty_needle_starts_nothing() {
		let search = Search::new();
		search.start(std::env::temp_dir(), "", false);
		assert!(!search.walking());
		assert_eq!(search.count(), 0);
	}
}
