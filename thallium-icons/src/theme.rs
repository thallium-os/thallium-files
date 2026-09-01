//! Icon-theme lookup on top of the XDG icon theme specification.
//!
//! Symbolic variants are preferred over full-colour ones. Thallium Files is a
//! graphite interface with a single accent, so a monochrome glyph the frontend
//! tints itself belongs in it; a full-colour icon set does not.
//!
//! Lookups happen on a worker. Resolving a type the caches have never seen
//! means walking several candidate names across four themes on disk, which
//! measured in the hundreds of milliseconds; done on the thread that draws, a
//! directory full of unfamiliar types froze the window outright. So a miss
//! answers `None` at once and queues the search, exactly as a thumbnail does,
//! and the icon appears on a later frame.

use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}, mpsc::{Sender, channel}}, thread};

/// Themes searched after the user's own, in order. `hicolor` is the
/// specification's mandatory fallback and is appended by the lookup itself.
const FALLBACK_THEMES: [&str; 2] = ["Adwaita", "breeze"];

/// Searches run at once. Lookups are disk-bound, and a directory of unfamiliar
/// types wants more than one in flight without flooding the machine.
const WORKERS: usize = 2;

/// What a search needs to run away from the caller.
struct Search {
	key:        String,
	candidates: Vec<String>,
}

pub struct IconTheme {
	/// The user's configured theme, when there is one.
	theme:     Option<String>,
	/// MIME type to resolved icon file. `None` records a miss so a type with no
	/// icon anywhere is not searched again on every repaint.
	resolved:  Arc<Mutex<HashMap<String, Option<PathBuf>>>>,
	/// Types already handed to a worker, so a repaint does not queue them twice.
	queued:    Arc<Mutex<HashSet<String>>>,
	/// Searches finished since the frontend last asked.
	completed: Arc<AtomicUsize>,
	requests:  Option<Sender<Search>>,
}

impl IconTheme {
	pub fn new(size: u16) -> Self {
		let theme = freedesktop_icons::default_theme_gtk();
		let resolved: Arc<Mutex<HashMap<String, Option<PathBuf>>>> = Arc::default();
		let queued: Arc<Mutex<HashSet<String>>> = Arc::default();
		let completed = Arc::new(AtomicUsize::new(0));
		let (sender, receiver) = channel::<Search>();
		let receiver = Arc::new(Mutex::new(receiver));

		for _ in 0..WORKERS {
			let receiver = Arc::clone(&receiver);
			let resolved = Arc::clone(&resolved);
			let queued = Arc::clone(&queued);
			let completed = Arc::clone(&completed);
			let theme = theme.clone();
			thread::spawn(move || {
				loop {
					let Ok(search) = ({
						let Ok(inbox) = receiver.lock() else { return };
						inbox.recv()
					}) else {
						return;
					};
					let found =
						search.candidates.into_iter().find_map(|name| lookup(theme.as_deref(), &name, size));
					if let Ok(mut cache) = resolved.lock() {
						cache.insert(search.key.clone(), found);
					}
					if let Ok(mut pending) = queued.lock() {
						pending.remove(&search.key);
					}
					completed.fetch_add(1, Ordering::Relaxed);
				}
			});
		}

		Self { theme, resolved, queued, completed, requests: Some(sender) }
	}

	/// The theme this lookup will search first, for diagnostics.
	pub fn name(&self) -> &str { self.theme.as_deref().unwrap_or("hicolor") }

	/// The icon file for `key`, or `None` while one is still being looked for.
	///
	/// Returns immediately in every case. The names are only computed on a
	/// miss: a repaint asks for every visible row, and almost every ask is a
	/// hit.
	pub fn find(&self, key: &str, candidates: impl FnOnce() -> Vec<String>) -> Option<PathBuf> {
		if let Some(hit) = self.resolved.lock().ok().and_then(|cache| cache.get(key).cloned()) {
			return hit;
		}
		let requests = self.requests.as_ref()?;
		let mut pending = self.queued.lock().ok()?;
		if !pending.insert(key.to_owned()) {
			return None;
		}
		let search = Search { key: key.to_owned(), candidates: candidates() };
		if requests.send(search).is_err() {
			pending.remove(key);
		}
		None
	}

	/// Whether any queued search finished since the last call, meaning the view
	/// has an icon it did not have before.
	pub fn take_completed(&self) -> usize { self.completed.swap(0, Ordering::Relaxed) }
}

/// Symbolic first, then the plain name; the user's theme first, then the
/// fallbacks. Searching symbolic across every theme before trying any plain
/// name would pull a stranger theme's glyph in ahead of the user's own.
fn lookup(configured: Option<&str>, name: &str, size: u16) -> Option<PathBuf> {
	let symbolic = format!("{name}-symbolic");
	let mut themes: Vec<&str> = Vec::with_capacity(3);
	if let Some(theme) = configured {
		themes.push(theme);
	}
	for fallback in FALLBACK_THEMES {
		if !themes.contains(&fallback) {
			themes.push(fallback);
		}
	}

	themes
		.into_iter()
		.find_map(|theme| in_theme(&symbolic, theme, size).or_else(|| in_theme(name, theme, size)))
}

fn in_theme(name: &str, theme: &str, size: u16) -> Option<PathBuf> {
	freedesktop_icons::lookup(name).with_size(size).with_theme(theme).with_cache().find()
}

#[cfg(test)]
mod tests {
	use std::{thread, time::Duration};

	use super::IconTheme;

	#[test]
	fn a_miss_is_recorded_so_it_is_searched_only_once() {
		let theme = IconTheme::new(24);
		let names = || vec!["thallium-icon-that-does-not-exist".to_owned()];
		// The first ask queues the search and answers immediately.
		assert_eq!(theme.find("test/absent", names), None);
		for _ in 0..200 {
			if theme.take_completed() > 0 {
				break;
			}
			thread::sleep(Duration::from_millis(10));
		}
		// The recorded miss is now served from the cache, still as a miss.
		assert_eq!(theme.find("test/absent", names), None);
		assert_eq!(theme.take_completed(), 0);
	}

	#[test]
	fn a_theme_name_is_always_reported() {
		assert!(!IconTheme::new(24).name().is_empty());
	}
}
