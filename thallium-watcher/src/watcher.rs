//! Watch-set coordination and notification reconciliation.
//!
//! Platform events are deliberately treated as lossy invalidation hints. Every
//! emitted delta is derived from a complete VFS scan compared with the last
//! authoritative snapshot, as selected in `PHASE2-PLAN.md` section 4.

use std::{collections::{BTreeMap, BTreeSet}, fmt, sync::{Arc, RwLock}};

use notify::{RecommendedWatcher, RecursiveMode, Watcher as _};
use thallium_fs_core::{BackendId, FsChange, FsDelta, FsError, FsErrorCode, FsResult, ListingGeneration, Location, NativePath};
use thallium_vfs::FileSystem;
use tokio::{sync::mpsc, time::Instant};

use crate::{NotificationBatch, NotificationBatcher, NotificationHint, Reporter, WatchSetDiff, WatchedSet, Watchee, WatcherConfig};

/// Last complete listing used as the baseline for one watched directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectorySnapshot {
	/// Generation assigned to the most recent completed reconciliation.
	pub generation: ListingGeneration,
	/// Complete VFS listing at that generation.
	pub entries:    Vec<thallium_fs_core::FileEntry>,
}

/// Owned output of one restat-and-reconcile pass.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReconcileResult {
	/// Normalized changes grouped and generation-tagged by parent location.
	pub deltas: Vec<FsDelta>,
}

/// Renderer- and state-neutral filesystem observation service.
pub struct Watcher<B: FileSystem> {
	backend:        Arc<B>,
	watched:        WatchedSet,
	config:         WatcherConfig,
	platform:       Option<RecommendedWatcher>,
	reporter:       Reporter,
	receiver:       mpsc::Receiver<NotificationHint>,
	batcher:        NotificationBatcher,
	snapshots:      BTreeMap<Location, DirectorySnapshot>,
	callback_roots: Arc<RwLock<BTreeSet<Location>>>,
	closed:         bool,
}

impl<B: FileSystem> fmt::Debug for Watcher<B> {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("Watcher")
			.field("watched", &self.watched)
			.field("config", &self.config)
			.field("snapshots", &self.snapshots)
			.field("closed", &self.closed)
			.finish_non_exhaustive()
	}
}

impl<B: FileSystem> Watcher<B> {
	/// Construct an inotify-backed watcher around an explicit VFS backend and
	/// bounded policy.
	pub fn new(backend: Arc<B>, config: WatcherConfig) -> FsResult<Self> {
		let config = config.validate()?;
		let (sender, receiver) = mpsc::channel(config.notification_capacity);
		let reporter = Reporter::new(sender);
		let callback_reporter = reporter.clone();
		let callback_roots: Arc<RwLock<BTreeSet<Location>>> = Arc::new(RwLock::new(BTreeSet::new()));
		let error_roots = Arc::clone(&callback_roots);
		let platform = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
			let paths = match event {
				Ok(event) if !event.paths.is_empty() => event.paths,
				Ok(_) | Err(_) => error_roots
					.read()
					.map(|roots| roots.iter().map(|root| root.path.as_path().to_path_buf()).collect())
					.unwrap_or_default(),
			};
			for path in paths {
				// Overflow is recorded by Reporter; the consumer then injects every
				// local root into the next authoritative reconciliation.
				let _ = callback_reporter.report(NotificationHint {
					location: Location::local(NativePath::from(path)).normalize(),
				});
			}
		})
		.map_err(|error| platform_error(error, None))?;

		Ok(Self {
			backend,
			watched: WatchedSet::default(),
			config,
			platform: Some(platform),
			reporter,
			receiver,
			batcher: NotificationBatcher::new(config),
			snapshots: BTreeMap::new(),
			callback_roots,
			closed: false,
		})
	}

	/// Synchronize native non-recursive watches with the complete desired set.
	///
	/// New watches are installed before their baseline scan, so an event racing
	/// with initialization remains queued for a subsequent authoritative rescan.
	pub async fn sync_watch_set(&mut self, desired: WatchedSet) -> FsResult<WatchSetDiff> {
		if self.closed {
			return Err(FsError::new(FsErrorCode::Cancelled, "watcher is shut down", None));
		}
		let desired = normalize_desired(desired)?;
		let diff = self.watched.diff(&desired);
		let mut installed = Vec::new();
		let mut baselines = BTreeMap::new();

		for watchee in &diff.added {
			let Watchee::Local { location, polling } = watchee else { continue };
			if *polling {
				self.rollback_installed(&installed);
				return Err(FsError::new(
					FsErrorCode::Unsupported,
					"polling watches are not implemented in the Linux MVP",
					Some(location.clone()),
				));
			}
			let platform = self.platform.as_mut().ok_or_else(|| {
				FsError::new(FsErrorCode::Cancelled, "watcher is shut down", Some(location.clone()))
			})?;
			if let Err(error) = platform.watch(location.path.as_path(), RecursiveMode::NonRecursive) {
				self.rollback_installed(&installed);
				return Err(platform_error(error, Some(location.clone())));
			}
			installed.push(location.clone());
			match self.backend.scan(location).await {
				Ok(entries) => {
					baselines.insert(location.clone(), DirectorySnapshot {
						generation: ListingGeneration(1),
						entries,
					});
				}
				Err(error) => {
					self.rollback_installed(&installed);
					return Err(error);
				}
			}
		}

		for watchee in &diff.removed {
			if let Watchee::Local { location, .. } = watchee {
				let platform = self.platform.as_mut().ok_or_else(|| {
					FsError::new(FsErrorCode::Cancelled, "watcher is shut down", Some(location.clone()))
				})?;
				platform
					.unwatch(location.path.as_path())
					.map_err(|error| platform_error(error, Some(location.clone())))?;
				self.snapshots.remove(location);
			}
		}

		self.snapshots.extend(baselines);
		self.watched = desired;
		self.replace_callback_roots();
		Ok(diff)
	}

	/// Wait for the next bounded, deduplicated batch. The debounce deadline is
	/// fixed from the first hint, so a continuous event stream cannot postpone a
	/// batch indefinitely.
	pub async fn next_batch(&mut self) -> Option<NotificationBatch> {
		if self.closed {
			return None;
		}
		let first = self.receiver.recv().await?;
		if let Some(batch) = self.batcher.push(first) {
			return Some(self.complete_batch(batch));
		}

		let deadline = tokio::time::sleep_until(Instant::now() + self.config.batch_window);
		tokio::pin!(deadline);
		loop {
			tokio::select! {
				() = &mut deadline => {
					return self.batcher.flush().map(|batch| self.complete_batch(batch));
				}
				hint = self.receiver.recv() => match hint {
					Some(hint) => {
						if let Some(batch) = self.batcher.push(hint) {
							return Some(self.complete_batch(batch));
						}
					}
					None => {
						return self.batcher.flush().map(|batch| self.complete_batch(batch));
					}
				}
			}
		}
	}

	/// Wait for one hint batch and reconcile it against authoritative VFS
	/// snapshots. `None` indicates a shut-down service.
	pub async fn next_deltas(&mut self) -> FsResult<Option<ReconcileResult>> {
		let Some(batch) = self.next_batch().await else { return Ok(None) };
		self.reconcile(batch).await.map(Some)
	}

	/// Rescan every watched local directory touched by a bounded hint batch and
	/// update its authoritative snapshot.
	pub async fn reconcile(&mut self, batch: NotificationBatch) -> FsResult<ReconcileResult> {
		let mut roots = self.affected_roots(&batch);
		if roots.is_empty() && !batch.hints.is_empty() {
			// Some backends report a path outside the watched root for a move. A
			// conservative complete refresh is preferable to missing a change.
			roots = self.local_roots();
		}

		let mut deltas = Vec::new();
		for location in roots {
			let previous = self.snapshots.get(&location).cloned().unwrap_or(DirectorySnapshot {
				generation: ListingGeneration::default(),
				entries:    Vec::new(),
			});
			let generation = previous.generation.next();
			match self.backend.scan(&location).await {
				Ok(entries) => {
					let changes = FsDelta::between(location.clone(), generation, &previous.entries, &entries);
					let retained_generation =
						if changes.is_empty() { previous.generation } else { generation };
					deltas.extend(changes);
					self
						.snapshots
						.insert(location, DirectorySnapshot { generation: retained_generation, entries });
				}
				Err(error) => {
					deltas.push(FsDelta {
						parent: location.clone(),
						generation,
						change: FsChange::Error(error),
					});
					self
						.snapshots
						.insert(location, DirectorySnapshot { generation, entries: previous.entries });
				}
			}
		}
		Ok(ReconcileResult { deltas })
	}

	/// Enqueue an explicit refresh request through the same bounded path used by
	/// the platform callback.
	pub fn request_refresh(&self, location: Location) -> Result<(), crate::ReportError> {
		self.reporter.report(NotificationHint { location })
	}

	/// Clone the bounded ingress endpoint for operation-completion hints.
	#[must_use]
	pub fn reporter(&self) -> Reporter { self.reporter.clone() }

	/// Stop accepting hints and release every native watch. Dropping a watcher
	/// also releases the platform handle, but explicit shutdown surfaces errors.
	pub fn shutdown(&mut self) -> FsResult<()> {
		if self.closed {
			return Ok(());
		}
		let mut first_error = None;
		let roots = self.local_roots();
		if let Some(platform) = self.platform.as_mut() {
			for location in roots {
				if let Err(error) = platform.unwatch(location.path.as_path()) {
					first_error.get_or_insert_with(|| platform_error(error, Some(location)));
				}
			}
		}
		self.platform.take();
		self.receiver.close();
		self.watched = WatchedSet::default();
		self.snapshots.clear();
		self.replace_callback_roots();
		self.closed = true;
		first_error.map_or(Ok(()), Err)
	}

	/// Return the currently active desired watch set.
	#[must_use]
	pub fn watched(&self) -> &WatchedSet { &self.watched }

	/// Return authoritative snapshots for diagnostics and recovery.
	#[must_use]
	pub fn snapshots(&self) -> &BTreeMap<Location, DirectorySnapshot> { &self.snapshots }

	/// Return the watcher batching and admission policy.
	#[must_use]
	pub const fn config(&self) -> WatcherConfig { self.config }

	/// Return the VFS backend used to derive authoritative observations.
	#[must_use]
	pub fn backend(&self) -> &Arc<B> { &self.backend }

	fn rollback_installed(&mut self, installed: &[Location]) {
		if let Some(platform) = self.platform.as_mut() {
			for location in installed {
				let _ = platform.unwatch(location.path.as_path());
			}
		}
	}

	fn local_roots(&self) -> BTreeSet<Location> {
		self
			.watched
			.entries
			.iter()
			.filter_map(|watchee| match watchee {
				Watchee::Local { location, .. } => Some(location.clone()),
				Watchee::Virtual { .. } => None,
			})
			.collect()
	}

	fn affected_roots(&self, batch: &NotificationBatch) -> BTreeSet<Location> {
		let roots = self.local_roots();
		roots
			.into_iter()
			.filter(|root| {
				batch.hints.iter().any(|hint| {
					hint.location.backend == root.backend
						&& hint.location.path.as_path().starts_with(root.path.as_path())
				})
			})
			.collect()
	}

	fn replace_callback_roots(&self) {
		if let Ok(mut roots) = self.callback_roots.write() {
			*roots = self.local_roots();
		}
	}

	fn complete_batch(&self, mut batch: NotificationBatch) -> NotificationBatch {
		if self.reporter.take_overflowed() {
			batch
				.hints
				.extend(self.local_roots().into_iter().map(|location| NotificationHint { location }));
			batch.hints.sort_unstable();
			batch.hints.dedup();
		}
		batch
	}
}

impl<B: FileSystem> Drop for Watcher<B> {
	fn drop(&mut self) {
		self.platform.take();
		self.receiver.close();
	}
}

fn platform_error(error: notify::Error, location: Option<Location>) -> FsError {
	FsError::new(FsErrorCode::Io, format!("filesystem watcher: {error}"), location)
}

fn normalize_desired(desired: WatchedSet) -> FsResult<WatchedSet> {
	let mut normalized = BTreeSet::new();
	for watchee in desired.entries {
		match watchee {
			Watchee::Local { location, polling } => {
				if location.backend != BackendId::LOCAL || !location.is_absolute() {
					return Err(FsError::new(
						FsErrorCode::InvalidInput,
						"local watches require an absolute location on the local backend",
						Some(location),
					));
				}
				normalized.insert(Watchee::Local { location: location.normalize(), polling });
			}
			Watchee::Virtual { location } => {
				normalized.insert(Watchee::Virtual { location });
			}
		}
	}
	Ok(WatchedSet { entries: normalized })
}

#[cfg(test)]
mod tests {
	use std::{sync::Arc, time::Duration};

	use tempfile::TempDir;
	use thallium_fs_core::{FsChange, ListingGeneration, Location, NativePath};
	use thallium_vfs::LocalBackend;

	use super::Watcher;
	use crate::{NotificationBatch, NotificationHint, WatchedSet, WatcherConfig};

	fn config() -> WatcherConfig {
		WatcherConfig {
			notification_capacity: 32,
			max_batch_size:        32,
			batch_window:          Duration::from_millis(40),
		}
	}

	async fn next_nonempty(watcher: &mut Watcher<LocalBackend>) -> super::ReconcileResult {
		tokio::time::timeout(Duration::from_secs(5), async {
			loop {
				let result = watcher.next_deltas().await.expect("reconcile").expect("watcher open");
				if !result.deltas.is_empty() {
					break result;
				}
			}
		})
		.await
		.expect("inotify event before timeout")
	}

	#[tokio::test]
	async fn watched_directory_changes_come_from_authoritative_scans() {
		let directory = TempDir::new().expect("temporary directory");
		let location = Location::local(NativePath::from(directory.path()));
		let mut watcher = Watcher::new(Arc::new(LocalBackend::new()), config()).expect("watcher");
		watcher.sync_watch_set(WatchedSet::single_local(location)).await.expect("install watch");

		let file = directory.path().join("created");
		tokio::fs::write(&file, b"content").await.expect("create file");
		let created = next_nonempty(&mut watcher).await;
		assert!(created.deltas.iter().any(|delta| matches!(delta.change, FsChange::Created(_))));

		tokio::fs::remove_file(file).await.expect("delete file");
		let deleted = next_nonempty(&mut watcher).await;
		assert!(deleted.deltas.iter().any(|delta| matches!(delta.change, FsChange::Deleted { .. })));
		watcher.shutdown().expect("shutdown");
	}

	#[tokio::test]
	async fn no_op_rescan_does_not_skip_the_next_observable_generation() {
		let directory = TempDir::new().expect("temporary directory");
		let location = Location::local(NativePath::from(directory.path()));
		let mut watcher = Watcher::new(Arc::new(LocalBackend::new()), config()).expect("watcher");
		watcher
			.sync_watch_set(WatchedSet::single_local(location.clone()))
			.await
			.expect("install watch");

		let unchanged = watcher
			.reconcile(NotificationBatch { hints: vec![NotificationHint { location: location.clone() }] })
			.await
			.expect("no-op reconcile");
		assert!(unchanged.deltas.is_empty());
		assert_eq!(watcher.snapshots()[&location].generation, ListingGeneration(1));

		let file = directory.path().join("later-change");
		tokio::fs::write(&file, b"content").await.expect("create file");
		let changed = watcher
			.reconcile(NotificationBatch {
				hints: vec![NotificationHint { location: Location::local(NativePath::from(file)) }],
			})
			.await
			.expect("changed reconcile");
		assert!(!changed.deltas.is_empty());
		assert!(changed.deltas.iter().all(|delta| delta.generation == ListingGeneration(2)));
		assert_eq!(watcher.snapshots()[&location].generation, ListingGeneration(2));
	}
}
