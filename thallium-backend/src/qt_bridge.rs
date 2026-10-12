//! Thin CXX-Qt adapter over owned snapshots from [`crate::Backend`].

use core::pin::Pin;
use std::{collections::BTreeSet, ffi::OsStr, os::unix::ffi::OsStrExt, path::PathBuf};

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use thallium_actions::Command;
use thallium_fs_core::{ConflictPolicy, EntryId, Location, NativePath};
use thallium_icons::{IconService, Thumbnail};
use thallium_protocol::{BrowserStateSnapshot, DirectoryEntryRow, PathBytes, TaskInfo, TaskStatus};
use thallium_rclone::{Remotes, authorize, config::{Provider, Session, Step}};

use crate::{Backend, BackendConfig, BackendResult, recents::Recent, search::Search, storage::Measure};

#[cxx_qt::bridge]
pub mod ffi {
	unsafe extern "C++" {
		include!("cxx-qt-lib/qstring.h");
		type QString = cxx_qt_lib::QString;
		include!("cxx-qt-lib/qlist.h");
		type QList_i32 = cxx_qt_lib::QList<i32>;
	}

	extern "RustQt" {
		#[qobject]
		#[qml_element]
		#[qproperty(QString, current_path, cxx_name = "currentPath")]
		#[qproperty(QString, home_path, cxx_name = "homePath")]
		#[qproperty(QString, last_error, cxx_name = "lastError")]
		#[qproperty(u64, model_revision, cxx_name = "modelRevision")]
		#[qproperty(bool, loading, cxx_name = "loading")]
		#[qproperty(bool, searching, cxx_name = "searching")]
		type BackendBridge = super::BackendBridgeRust;

		#[qinvokable]
		fn poll(self: Pin<&mut Self>);
		#[qinvokable]
		fn navigate(self: Pin<&mut Self>, path: &QString);
		#[qinvokable]
		#[rust_name = "navigate_row"]
		fn navigateRow(self: Pin<&mut Self>, row: i32);
		#[qinvokable]
		fn up(self: Pin<&mut Self>);
		#[qinvokable]
		fn back(self: Pin<&mut Self>);
		#[qinvokable]
		fn forward(self: Pin<&mut Self>);
		#[qinvokable]
		fn refresh(self: Pin<&mut Self>);

		#[qinvokable]
		#[rust_name = "entry_count"]
		fn entryCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "entry_name"]
		fn entryName(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "entry_path"]
		fn entryPath(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "entry_detail"]
		fn entryDetail(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "entry_modified"]
		fn entryModified(&self, row: i32) -> i64;
		#[qinvokable]
		#[rust_name = "entry_is_directory"]
		fn entryIsDirectory(&self, row: i32) -> bool;
		#[qinvokable]
		#[rust_name = "entry_icon"]
		fn entryIcon(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "entry_thumbnail"]
		fn entryThumbnail(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "entry_selected"]
		fn entrySelected(&self, row: i32) -> bool;
		#[qinvokable]
		#[rust_name = "toggle_selection"]
		fn toggleSelection(self: Pin<&mut Self>, row: i32);
		#[qinvokable]
		#[rust_name = "selection_count"]
		fn selectionCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "clear_selection"]
		fn clearSelection(self: Pin<&mut Self>);
		#[qinvokable]
		#[rust_name = "select_rows"]
		fn selectRows(self: Pin<&mut Self>, rows: &QList_i32);
		#[qinvokable]
		#[rust_name = "open_link"]
		fn openLink(&self, url: &QString);
		#[qinvokable]
		#[rust_name = "open_entry"]
		fn openEntry(&self, row: i32);
		#[qinvokable]
		#[rust_name = "start_search"]
		fn startSearch(self: Pin<&mut Self>, needle: &QString, include_hidden: bool);
		#[qinvokable]
		#[rust_name = "cancel_search"]
		fn cancelSearch(self: Pin<&mut Self>);
		#[qinvokable]
		#[rust_name = "search_count"]
		fn searchCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "search_name"]
		fn searchName(&self, index: i32) -> QString;
		#[qinvokable]
		#[rust_name = "search_path"]
		fn searchPath(&self, index: i32) -> QString;
		#[qinvokable]
		#[rust_name = "search_folder"]
		fn searchFolder(&self, index: i32) -> QString;
		#[qinvokable]
		#[rust_name = "search_is_directory"]
		fn searchIsDirectory(&self, index: i32) -> bool;
		#[qinvokable]
		#[rust_name = "search_truncated"]
		fn searchTruncated(&self) -> bool;
		#[qinvokable]
		#[rust_name = "open_path"]
		fn openPath(&self, path: &QString);
		#[qinvokable]
		#[rust_name = "refresh_recents"]
		fn refreshRecents(self: Pin<&mut Self>);
		#[qinvokable]
		#[rust_name = "recent_count"]
		fn recentCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "recent_name"]
		fn recentName(&self, index: i32) -> QString;
		#[qinvokable]
		#[rust_name = "recent_path"]
		fn recentPath(&self, index: i32) -> QString;
		#[qinvokable]
		#[rust_name = "recent_folder"]
		fn recentFolder(&self, index: i32) -> QString;
		#[qinvokable]
		#[rust_name = "storage_total"]
		fn storageTotal(&self) -> f64;
		#[qinvokable]
		#[rust_name = "storage_used"]
		fn storageUsed(&self) -> f64;
		#[qinvokable]
		#[rust_name = "storage_known"]
		fn storageKnown(&self) -> bool;
		#[qinvokable]
		#[rust_name = "storage_share"]
		fn storageShare(&self) -> f64;
		#[qinvokable]
		#[rust_name = "folder_bytes"]
		fn folderBytes(&self) -> f64;
		#[qinvokable]
		#[rust_name = "folder_files"]
		fn folderFiles(&self) -> f64;
		#[qinvokable]
		#[rust_name = "folder_measuring"]
		fn folderMeasuring(&self) -> bool;
		#[qinvokable]
		#[rust_name = "folder_share"]
		fn folderShare(&self) -> f64;
		#[qinvokable]
		#[rust_name = "config_waiting"]
		fn configWaiting(&self) -> bool;

		#[qinvokable]
		#[rust_name = "copy_selected"]
		fn copySelected(self: Pin<&mut Self>, destination: &QString);
		#[qinvokable]
		#[rust_name = "move_selected"]
		fn moveSelected(self: Pin<&mut Self>, destination: &QString);
		#[qinvokable]
		#[rust_name = "delete_selected"]
		fn deleteSelected(self: Pin<&mut Self>);
		#[qinvokable]
		#[rust_name = "trash_selected"]
		fn trashSelected(self: Pin<&mut Self>);
		#[qinvokable]
		#[rust_name = "rename_row"]
		fn renameRow(self: Pin<&mut Self>, row: i32, name: &QString);
		#[qinvokable]
		#[rust_name = "create_folder"]
		fn createFolder(self: Pin<&mut Self>, name: &QString);
		#[qinvokable]
		#[rust_name = "create_file"]
		fn createFile(self: Pin<&mut Self>, name: &QString, source: &QString);
		#[qinvokable]
		#[rust_name = "link_selected"]
		fn linkSelected(self: Pin<&mut Self>, destination: &QString);
		#[qinvokable]
		#[rust_name = "free_name"]
		fn freeName(&self, base: &QString) -> QString;
		#[qinvokable]
		#[rust_name = "template_count"]
		fn templateCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "template_name"]
		fn templateName(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "template_path"]
		fn templatePath(&self, row: i32) -> QString;

		#[qinvokable]
		#[rust_name = "remotes_configurable"]
		fn remotesConfigurable(&self) -> bool;
		#[qinvokable]
		#[rust_name = "load_providers"]
		fn loadProviders(self: Pin<&mut Self>);
		#[qinvokable]
		#[rust_name = "provider_count"]
		fn providerCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "provider_label"]
		fn providerLabel(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "provider_kind"]
		fn providerKind(&self, row: i32) -> QString;

		#[qinvokable]
		#[rust_name = "begin_remote"]
		fn beginRemote(self: Pin<&mut Self>, name: &QString, kind: &QString);
		#[qinvokable]
		#[rust_name = "answer_remote"]
		fn answerRemote(self: Pin<&mut Self>, response: &QString);
		#[qinvokable]
		#[rust_name = "cancel_remote"]
		fn cancelRemote(self: Pin<&mut Self>);
		#[qinvokable]
		#[rust_name = "forget_remote"]
		fn forgetRemote(self: Pin<&mut Self>, name: &QString);

		#[qinvokable]
		#[rust_name = "config_active"]
		fn configActive(&self) -> bool;
		#[qinvokable]
		#[rust_name = "config_field"]
		fn configField(&self) -> QString;
		#[qinvokable]
		#[rust_name = "config_help"]
		fn configHelp(&self) -> QString;
		#[qinvokable]
		#[rust_name = "config_default"]
		fn configDefault(&self) -> QString;
		#[qinvokable]
		#[rust_name = "config_secret"]
		fn configSecret(&self) -> bool;
		#[qinvokable]
		#[rust_name = "config_link"]
		fn configLink(&self) -> QString;
		#[qinvokable]
		#[rust_name = "config_choice_count"]
		fn configChoiceCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "config_choice_value"]
		fn configChoiceValue(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "config_choice_help"]
		fn configChoiceHelp(&self, row: i32) -> QString;

		#[qinvokable]
		#[rust_name = "remote_count"]
		fn remoteCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "remote_name"]
		fn remoteName(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "remote_location"]
		fn remoteLocation(&self, row: i32) -> QString;

		#[qinvokable]
		#[rust_name = "task_count"]
		fn taskCount(&self) -> i32;
		#[qinvokable]
		#[rust_name = "task_id"]
		fn taskId(&self, row: i32) -> u64;
		#[qinvokable]
		#[rust_name = "task_title"]
		fn taskTitle(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "task_status"]
		fn taskStatus(&self, row: i32) -> QString;
		#[qinvokable]
		#[rust_name = "task_progress"]
		fn taskProgress(&self, row: i32) -> i32;
		#[qinvokable]
		#[rust_name = "task_active"]
		fn taskActive(&self, row: i32) -> bool;
		#[qinvokable]
		#[rust_name = "cancel_task"]
		fn cancelTask(self: Pin<&mut Self>, id: u64);
	}
}

pub struct BackendBridgeRust {
	current_path:   QString,
	home_path:      QString,
	last_error:     QString,
	model_revision: u64,
	/// Whether a navigation is still out with the owner service.
	loading:        bool,
	/// Whether a recursive search is still walking.
	searching:      bool,
	backend:        Option<Backend>,
	snapshot:       BrowserStateSnapshot,
	icons:          IconService,
	/// Storage systems rclone offers, read once when the panel is opened.
	providers:      Vec<Provider>,
	/// The configuration conversation in progress, if any.
	session:        Option<Session>,
	/// The browser consent flow in progress, if the conversation reached one.
	consent:        Option<Consent>,
	/// Polls since the last full reconcile; see [`RECONCILE_TICKS`].
	idle_ticks:     u32,
	/// The outstanding navigation, collected on a later poll.
	pending:        Option<tokio::sync::oneshot::Receiver<Result<(), String>>>,
	/// Recursive name search over the local filesystem.
	search:         Search,
	/// The desktop's recent-files list, re-read when the panel is opened.
	recents:        Vec<Recent>,
	/// A running count of what the open directory holds.
	measure:        Measure,
	/// The directory `measure` was last started for, so navigating restarts it
	/// and a redraw does not.
	measured:       String,
}

/// Polls between full reconciles, at the frontend's 100ms poll interval.
///
/// Deltas say when the state has moved, so the snapshot is normally re-read
/// only when one arrives. They are advisory and dropped when their channel
/// fills, though, so a reconcile still runs on its own: often enough to recover
/// a lost delta, seldom enough that an idle window is not cloning and comparing
/// the whole directory ten times a second.
const RECONCILE_TICKS: u32 = 20;

/// A consent flow and what the frontend has already done about it.
struct Consent {
	progress: authorize::Shared,
	/// The link is opened once, when it first appears.
	opened:   bool,
}

impl Default for BackendBridgeRust {
	fn default() -> Self {
		match Backend::start(BackendConfig::default()) {
			Ok(backend) => {
				let snapshot = backend.snapshot().unwrap_or_default();
				let current_path = snapshot
					.current_location
					.as_ref()
					.map_or_else(String::new, |location| location.path.display_lossy.clone());
				Self {
					current_path: current_path.clone().into(),
					home_path: current_path.into(),
					last_error: QString::default(),
					model_revision: snapshot.revision.0,
					backend: Some(backend),
					snapshot,
					icons: IconService::new(),
					providers: Vec::new(),
					session: None,
					consent: None,
					idle_ticks: 0,
					loading: false,
					searching: false,
					pending: None,
					search: Search::new(),
					recents: Vec::new(),
					measure: Measure::new(),
					measured: String::new(),
				}
			}
			Err(error) => Self {
				current_path:   QString::default(),
				home_path:      QString::default(),
				last_error:     error.to_string().into(),
				model_revision: 0,
				backend:        None,
				snapshot:       BrowserStateSnapshot::default(),
				icons:          IconService::new(),
				providers:      Vec::new(),
				session:        None,
				consent:        None,
				idle_ticks:     0,
				loading:        false,
				searching:      false,
				pending:        None,
				search:         Search::new(),
				recents:        Vec::new(),
				measure:        Measure::new(),
				measured:       String::new(),
			},
		}
	}
}

impl ffi::BackendBridge {
	pub fn poll(mut self: Pin<&mut Self>) {
		let mut moved = false;
		if let Some(backend) = self.as_mut().rust_mut().backend.as_mut() {
			while backend.next_event().is_ok_and(|event| event.is_some()) {
				moved = true;
			}
		}
		self.as_mut().carry_consent();
		// A finished thumbnail changes what the view should draw without
		// changing the snapshot, so it needs a revision of its own.
		let thumbnails_arrived = self.rust().icons.poll_thumbnails();

		let settled = self.as_mut().collect_pending();

		// Navigating starts a new count of what the directory holds.
		self.as_mut().measure_folder();
		let counting = self.rust().measure.walking();

		// A search publishes as it walks, so the view is told whenever results
		// have arrived and again when the walk stops.
		let found = self.rust().search.take_arrived() > 0;
		let walking = self.rust().search.walking();
		let was_walking = *self.searching();
		if walking != was_walking {
			self.as_mut().set_searching(walking);
		}
		// A count in progress changes the sidebar on every tick until it stops.
		if found || walking != was_walking || counting {
			let revision = self.model_revision().saturating_add(1);
			self.as_mut().set_model_revision(revision);
		}

		let ticks = self.rust().idle_ticks.saturating_add(1);
		let reconcile = ticks >= RECONCILE_TICKS;
		self.as_mut().rust_mut().idle_ticks = if reconcile { 0 } else { ticks };
		if moved || settled || reconcile {
			self.as_mut().reload();
		}
		if thumbnails_arrived {
			let revision = self.model_revision().saturating_add(1);
			self.as_mut().set_model_revision(revision);
		}
	}

	pub fn navigate(mut self: Pin<&mut Self>, path: &QString) {
		let location = self.as_mut().location(path);
		self.submit_detached(Command::Navigate(location));
	}

	pub fn navigate_row(self: Pin<&mut Self>, row: i32) {
		let location =
			self.row(row).filter(|entry| entry.is_directory()).map(|entry| entry.location.to_location());
		if let Some(location) = location {
			self.submit_detached(Command::Navigate(location));
		}
	}

	pub fn up(self: Pin<&mut Self>) {
		let parent = self
			.rust()
			.snapshot
			.current_location
			.as_ref()
			.and_then(|location| location.to_location().parent());
		if let Some(parent) = parent {
			self.submit_detached(Command::Navigate(parent));
		}
	}

	pub fn back(self: Pin<&mut Self>) { self.submit_detached(Command::NavigateBack); }

	pub fn forward(self: Pin<&mut Self>) { self.submit_detached(Command::NavigateForward); }

	/// Re-read the directory, and count what it holds again.
	///
	/// A count stands for a while once made, so refreshing is how the size is
	/// asked for afresh rather than waiting for that to lapse.
	pub fn refresh(mut self: Pin<&mut Self>) {
		if let Some(location) = self.rust().snapshot.current_location.as_ref()
			&& location.backend_id == 0
		{
			let path = native_path(&location.path);
			self.rust().measure.forget(&path);
			self.as_mut().rust_mut().measured = String::new();
		}
		self.submit_detached(Command::Refresh);
	}

	pub fn entry_count(&self) -> i32 { count(self.rust().snapshot.directory.entries.len()) }

	pub fn entry_name(&self, row: i32) -> QString {
		self.row(row).map_or_else(QString::default, |entry| entry.name.display_lossy.clone().into())
	}

	pub fn entry_path(&self, row: i32) -> QString {
		self.row(row).map_or_else(QString::default, |entry| self.display(Some(&entry.location)).into())
	}

	pub fn entry_detail(&self, row: i32) -> QString {
		self.row(row).map_or_else(QString::default, |entry| {
			let detail = if entry.is_directory() { "Folder".into() } else { format_size(entry.size) };
			detail.into()
		})
	}

	pub fn entry_modified(&self, row: i32) -> i64 {
		self.row(row).and_then(|entry| entry.modified).map_or(0, |modified| modified.seconds)
	}

	pub fn entry_is_directory(&self, row: i32) -> bool {
		self.row(row).is_some_and(DirectoryEntryRow::is_directory)
	}

	pub fn entry_icon(&self, row: i32) -> QString {
		self.row(row).map_or_else(QString::default, |entry| {
			let path = native_path(&entry.location.path);
			let mime = self.rust().icons.mime_of(&path, entry.is_directory());
			self
				.rust()
				.icons
				.icon(&mime)
				.map_or_else(QString::default, |icon| thallium_icons::file_url(&icon).into())
		})
	}

	pub fn entry_thumbnail(&self, row: i32) -> QString {
		self.row(row).map_or_else(QString::default, |entry| {
			// A remote entry has no local file to read; asking for a thumbnail
			// would download it in full just to shrink it.
			if entry.is_directory() || entry.location.backend_id != 0 {
				return QString::default();
			}
			let path = native_path(&entry.location.path);
			let mime = self.rust().icons.mime_of(&path, false);
			let modified = entry.modified.map_or(0, |modified| modified.seconds);
			match self.rust().icons.thumbnail(&path, &mime, modified) {
				Thumbnail::Ready(file) => thallium_icons::file_url(&file).into(),
				Thumbnail::Pending | Thumbnail::Unavailable => QString::default(),
			}
		})
	}

	pub fn entry_selected(&self, row: i32) -> bool {
		self.row(row).is_some_and(|entry| self.rust().snapshot.selection.contains(&entry.id))
	}

	pub fn toggle_selection(self: Pin<&mut Self>, row: i32) {
		if let Some(id) = self.row(row).map(|entry| entry.id.to_entry_id()) {
			self.submit(Command::ToggleSelection(id));
		}
	}

	pub fn selection_count(&self) -> i32 { count(self.rust().snapshot.selection.len()) }

	pub fn clear_selection(self: Pin<&mut Self>) { self.submit(Command::ClearSelection); }

	/// Replace the whole selection in one command. A drag-selection changes
	/// every covered entry at once, and submitting a toggle apiece would rebuild
	/// the snapshot once per entry.
	pub fn select_rows(self: Pin<&mut Self>, rows: &ffi::QList_i32) {
		let mut selected = Vec::with_capacity(rows.len().max(0) as usize);
		for row in rows.iter() {
			if let Some(entry) = self.row(*row) {
				selected.push(entry.id.to_entry_id());
			}
		}
		let focused = selected.last().copied();
		self.submit(Command::SetSelection { selected, focused });
	}

	pub fn copy_selected(mut self: Pin<&mut Self>, destination: &QString) {
		let sources = self.selected_locations();
		let destination = self.as_mut().location(destination);
		self.submit(Command::Copy { sources, destination, conflict_policy: ConflictPolicy::Rename });
	}

	pub fn move_selected(mut self: Pin<&mut Self>, destination: &QString) {
		let sources = self.selected_locations();
		let destination = self.as_mut().location(destination);
		self.submit(Command::Move { sources, destination, conflict_policy: ConflictPolicy::Rename });
	}

	pub fn rename_row(self: Pin<&mut Self>, row: i32, name: &QString) {
		let Some(source) = self.row(row).map(|entry| entry.location.to_location()) else {
			return;
		};
		let name = NativePath::new(String::from(name).into_bytes());
		self.submit(Command::Rename { source, name });
	}

	pub fn create_folder(self: Pin<&mut Self>, name: &QString) {
		let Some(parent) = self.current() else { return };
		let name = NativePath::new(String::from(name).into_bytes());
		self.submit(Command::CreateFolder { parent, name });
	}

	pub fn create_file(self: Pin<&mut Self>, name: &QString, source: &QString) {
		let Some(parent) = self.current() else { return };
		let name = NativePath::new(String::from(name).into_bytes());
		let template = (!source.is_empty()).then(|| self.location(source));
		self.submit(Command::CreateFile { parent, name, template });
	}

	pub fn link_selected(mut self: Pin<&mut Self>, destination: &QString) {
		let sources = self.selected_locations();
		let destination = self.as_mut().location(destination);
		self.submit(Command::Link { sources, destination });
	}

	/// `base`, or `base 2`, `base 3`, ... whichever is not yet taken in the
	/// open folder. An extension stays at the end: `Untitled 2.txt`.
	pub fn free_name(&self, base: &QString) -> QString {
		let base = String::from(base);
		let taken: BTreeSet<&[u8]> = self
			.rust()
			.snapshot
			.directory
			.entries
			.iter()
			.map(|entry| entry.name.bytes.as_slice())
			.collect();
		let (stem, extension) = match base.rfind('.') {
			Some(dot) if dot > 0 => base.split_at(dot),
			_ => (base.as_str(), ""),
		};
		let mut candidate = base.clone();
		for number in 2.. {
			if !taken.contains(candidate.as_bytes()) {
				break;
			}
			candidate = format!("{stem} {number}{extension}");
		}
		candidate.into()
	}

	pub fn template_count(&self) -> i32 { count(templates().len()) }

	pub fn template_name(&self, row: i32) -> QString {
		template(row).map_or_else(QString::default, |path| {
			path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()).into()
		})
	}

	pub fn template_path(&self, row: i32) -> QString {
		template(row).map_or_else(QString::default, |path| path.to_string_lossy().as_ref().into())
	}

	pub fn delete_selected(self: Pin<&mut Self>) {
		let targets = self.selected_locations();
		self.submit(Command::Delete { targets });
	}

	pub fn trash_selected(self: Pin<&mut Self>) {
		let targets = self.selected_locations();
		self.submit(Command::Trash { targets });
	}

	pub fn remotes_configurable(&self) -> bool {
		self.rust().backend.as_ref().is_some_and(Backend::can_configure_remotes)
	}

	pub fn load_providers(mut self: Pin<&mut Self>) {
		if !self.rust().providers.is_empty() {
			return;
		}
		let listed = self.rust().backend.as_ref().map(Backend::remote_providers);
		match listed {
			Some(Ok(providers)) => self.as_mut().rust_mut().providers = providers,
			Some(Err(error)) => self.as_mut().set_last_error(error.to_string().into()),
			None => {}
		}
	}

	pub fn provider_count(&self) -> i32 { count(self.rust().providers.len()) }

	pub fn provider_label(&self, row: i32) -> QString {
		self.provider(row).map_or_else(QString::default, |provider| provider.description.clone().into())
	}

	pub fn provider_kind(&self, row: i32) -> QString {
		self.provider(row).map_or_else(QString::default, |provider| provider.kind.clone().into())
	}

	/// Start configuring a remote. The first question, if any, is available
	/// immediately afterwards.
	pub fn begin_remote(self: Pin<&mut Self>, name: &QString, kind: &QString) {
		let started = self
			.rust()
			.backend
			.as_ref()
			.map(|backend| backend.begin_remote(&String::from(name), &String::from(kind)));
		self.take_up(started);
	}

	pub fn answer_remote(self: Pin<&mut Self>, response: &QString) {
		let Some(session) = self.rust().session.clone() else { return };
		let answered = self
			.rust()
			.backend
			.as_ref()
			.map(|backend| backend.answer_remote(&session, &String::from(response)));
		self.take_up(answered);
	}

	/// Run `rclone authorize` when the conversation reaches its token question.
	fn begin_consent(mut self: Pin<&mut Self>) {
		if self.rust().consent.is_some()
			|| !self.rust().session.as_ref().is_some_and(Session::wants_token)
		{
			return;
		}
		let Some(kind) = self.rust().session.as_ref().map(|session| session.kind().to_owned()) else {
			return;
		};
		let started = self.rust().backend.as_ref().map(|backend| backend.begin_consent(&kind));
		match started {
			Some(Ok(progress)) => {
				self.as_mut().rust_mut().consent = Some(Consent { progress, opened: false });
			}
			Some(Err(error)) => self.as_mut().set_last_error(error.to_string().into()),
			None => {}
		}
	}

	/// Open the consent link once it exists, and answer with the token once the
	/// browser has come back. Called on every poll, so it must stay cheap and
	/// must do each of those things exactly once.
	fn carry_consent(mut self: Pin<&mut Self>) {
		let Some(consent) = self.rust().consent.as_ref() else { return };
		let Ok(progress) = consent.progress.lock().map(|state| state.clone()) else { return };

		if !progress.error.is_empty() {
			self.as_mut().rust_mut().consent = None;
			self.as_mut().set_last_error(progress.error.into());
			return;
		}

		if !progress.url.is_empty() && !consent.opened {
			if let Some(consent) = self.as_mut().rust_mut().consent.as_mut() {
				consent.opened = true;
			}
			let url = QString::from(&progress.url);
			self.as_ref().open_link(&url);
			let revision = self.model_revision().saturating_add(1);
			self.as_mut().set_model_revision(revision);
		}

		if !progress.token.is_empty() {
			self.as_mut().rust_mut().consent = None;
			let token = QString::from(&progress.token);
			self.answer_remote(&token);
		}
	}

	/// Abandon the conversation and remove what rclone already wrote.
	///
	/// The remote exists in the configuration from the first call, before any
	/// question has been answered, so walking away has to take it back out;
	/// otherwise cancelling leaves a half-configured remote in the sidebar.
	pub fn cancel_remote(mut self: Pin<&mut Self>) {
		let Some(name) = self.rust().session.as_ref().map(|session| session.name().to_owned()) else {
			return;
		};
		self.as_mut().rust_mut().session = None;
		// Dropping the handle kills the `rclone authorize` child with it.
		self.as_mut().rust_mut().consent = None;
		let result = self
			.rust()
			.backend
			.as_ref()
			.map(|backend| backend.forget_remote(&name).and_then(|()| backend.refresh_remotes()));
		if let Some(Err(error)) = result {
			self.as_mut().set_last_error(error.to_string().into());
		}
		let revision = self.model_revision().saturating_add(1);
		self.as_mut().set_model_revision(revision);
	}

	pub fn forget_remote(mut self: Pin<&mut Self>, name: &QString) {
		let name = String::from(name);
		let result = self
			.rust()
			.backend
			.as_ref()
			.map(|backend| backend.forget_remote(&name).and_then(|()| backend.refresh_remotes()));
		match result {
			Some(Err(error)) => self.as_mut().set_last_error(error.to_string().into()),
			Some(Ok(())) => {
				self.as_mut().set_last_error(QString::default());
				let revision = self.model_revision().saturating_add(1);
				self.as_mut().set_model_revision(revision);
			}
			None => {}
		}
	}

	pub fn config_active(&self) -> bool { self.question().is_some() }

	pub fn config_field(&self) -> QString {
		self.question().map_or_else(QString::default, |question| question.name.clone().into())
	}

	pub fn config_help(&self) -> QString {
		self.question().map_or_else(QString::default, |question| question.help.clone().into())
	}

	pub fn config_default(&self) -> QString {
		self.question().map_or_else(QString::default, |question| question.default.clone().into())
	}

	pub fn config_secret(&self) -> bool { self.question().is_some_and(|question| question.secret) }

	/// The authorisation link rclone put in its help text, when there is one.
	/// The link the user should open. While consent is running this is the real
	/// authorisation URL; rclone's own help for that step only carries a link to
	/// its documentation, which is no use to anyone trying to log in.
	pub fn config_link(&self) -> QString {
		if let Some(url) = self.consent_url() {
			return QString::from(&url);
		}
		self
			.question()
			.and_then(|question| thallium_rclone::config::link_in(&question.help))
			.map_or_else(QString::default, QString::from)
	}

	/// Whether the interface is waiting on the browser rather than on the user.
	pub fn config_waiting(&self) -> bool { self.rust().consent.is_some() }

	fn consent_url(&self) -> Option<String> {
		let consent = self.rust().consent.as_ref()?;
		let url = consent.progress.lock().ok()?.url.clone();
		(!url.is_empty()).then_some(url)
	}

	/// Walk the current directory for names containing `needle`.
	///
	/// Local only: a remote would be one network round trip per directory, which
	/// is a different problem with a different answer.
	pub fn start_search(mut self: Pin<&mut Self>, needle: &QString, include_hidden: bool) {
		let root = self
			.rust()
			.snapshot
			.current_location
			.as_ref()
			.filter(|location| location.backend_id == 0)
			.map(|location| native_path(&location.path));
		let needle = String::from(needle);
		let Some(root) = root else { return };
		self.rust().search.start(root, &needle, include_hidden);
		let walking = self.rust().search.walking();
		self.as_mut().set_searching(walking);
		let revision = self.model_revision().saturating_add(1);
		self.as_mut().set_model_revision(revision);
	}

	pub fn cancel_search(mut self: Pin<&mut Self>) {
		self.rust().search.cancel();
		self.as_mut().set_searching(false);
		let revision = self.model_revision().saturating_add(1);
		self.as_mut().set_model_revision(revision);
	}

	pub fn search_count(&self) -> i32 { count(self.rust().search.count()) }

	pub fn search_name(&self, index: i32) -> QString {
		self.hit(index).map_or_else(QString::default, |hit| QString::from(&hit.name))
	}

	pub fn search_path(&self, index: i32) -> QString {
		self
			.hit(index)
			.map_or_else(QString::default, |hit| QString::from(&hit.path.to_string_lossy().into_owned()))
	}

	/// The directory holding a result, which is where opening it navigates to.
	pub fn search_folder(&self, index: i32) -> QString {
		self.hit(index).map_or_else(QString::default, |hit| {
			let folder =
				hit.path.parent().map_or_else(String::new, |at| at.to_string_lossy().into_owned());
			QString::from(&folder)
		})
	}

	pub fn search_is_directory(&self, index: i32) -> bool {
		self.hit(index).is_some_and(|hit| hit.is_directory)
	}

	pub fn search_truncated(&self) -> bool { self.rust().search.truncated() }

	fn hit(&self, index: i32) -> Option<crate::search::Hit> {
		usize::try_from(index).ok().and_then(|index| self.rust().search.hit(index))
	}

	/// Re-read the desktop's recent-files list.
	///
	/// Read on demand rather than watched: other applications append to it
	/// constantly, and the panel is only looked at now and then.
	pub fn refresh_recents(mut self: Pin<&mut Self>) {
		let home = PathBuf::from(String::from(self.home_path()));
		let found = crate::recents::read(&home);
		self.as_mut().rust_mut().recents = found;
		let revision = self.model_revision().saturating_add(1);
		self.as_mut().set_model_revision(revision);
	}

	pub fn recent_count(&self) -> i32 { count(self.rust().recents.len()) }

	pub fn recent_name(&self, index: i32) -> QString {
		self.recent(index).map_or_else(QString::default, |entry| QString::from(&entry.name))
	}

	pub fn recent_path(&self, index: i32) -> QString {
		self.recent(index).map_or_else(QString::default, |entry| {
			QString::from(&entry.path.to_string_lossy().into_owned())
		})
	}

	pub fn recent_folder(&self, index: i32) -> QString {
		self.recent(index).map_or_else(QString::default, |entry| {
			let folder =
				entry.path.parent().map_or_else(String::new, |at| at.to_string_lossy().into_owned());
			QString::from(&folder)
		})
	}

	fn recent(&self, index: i32) -> Option<&Recent> {
		usize::try_from(index).ok().and_then(|index| self.rust().recents.get(index))
	}

	/// Capacity of the filesystem the open directory lives on. A remote reports
	/// nothing: its capacity is not this machine's to describe.
	fn usage(&self) -> Option<crate::storage::Usage> {
		let location = self.rust().snapshot.current_location.as_ref()?;
		if location.backend_id != 0 {
			return None;
		}
		crate::storage::of(&native_path(&location.path))
	}

	pub fn storage_total(&self) -> f64 { self.usage().map_or(0.0, |usage| usage.total as f64) }

	pub fn storage_used(&self) -> f64 { self.usage().map_or(0.0, |usage| usage.used() as f64) }

	pub fn storage_known(&self) -> bool { self.usage().is_some_and(|usage| usage.total > 0) }

	/// How full the disk is, as a fraction. Computed here rather than in the
	/// frontend so the division has one home and one guard against a zero size.
	pub fn storage_share(&self) -> f64 { self.usage().map_or(0.0, |usage| usage.share_used()) }

	pub fn folder_bytes(&self) -> f64 { self.rust().measure.bytes() as f64 }

	pub fn folder_files(&self) -> f64 { self.rust().measure.files() as f64 }

	pub fn folder_measuring(&self) -> bool { self.rust().measure.walking() }

	/// What the open directory takes of the disk it is on. This is what the ring
	/// draws: the folder against its filesystem, not the filesystem alone.
	pub fn folder_share(&self) -> f64 {
		let total = self.usage().map_or(0, |usage| usage.total);
		if total == 0 {
			return 0.0;
		}
		(self.rust().measure.bytes() as f64 / total as f64).clamp(0.0, 1.0)
	}

	/// Count what the open directory holds, once per directory.
	///
	/// A remote is not counted: every subdirectory would be another network
	/// round trip, which is not worth a number in the sidebar.
	fn measure_folder(mut self: Pin<&mut Self>) {
		let local = self
			.rust()
			.snapshot
			.current_location
			.as_ref()
			.filter(|location| location.backend_id == 0)
			.map(|location| native_path(&location.path));
		let here = local.as_ref().map_or_else(String::new, |path| path.to_string_lossy().into_owned());
		if here == self.rust().measured {
			return;
		}
		self.as_mut().rust_mut().measured = here;
		match local {
			Some(path) => self.rust().measure.start(path),
			None => self.rust().measure.cancel(),
		}
	}

	/// Hand an arbitrary local path to the desktop.
	pub fn open_path(&self, path: &QString) {
		let path = PathBuf::from(String::from(path));
		if !path.is_absolute() {
			return;
		}
		let _ = std::process::Command::new("xdg-open")
			.arg(&path)
			.stdin(std::process::Stdio::null())
			.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::null())
			.spawn();
	}

	/// Hand a file to the desktop to open with whatever handles its type.
	///
	/// A remote entry has no local path to hand over; opening one would mean
	/// downloading it first, which is a decision for the user, not a menu item.
	pub fn open_entry(&self, row: i32) {
		let Some(entry) = self.row(row) else { return };
		if entry.location.backend_id != 0 {
			return;
		}
		let path = native_path(&entry.location.path);
		let _ = std::process::Command::new("xdg-open")
			.arg(&path)
			.stdin(std::process::Stdio::null())
			.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::null())
			.spawn();
	}

	/// Hand a URL to the desktop. Consent happens in the user's own browser,
	/// signed into their own account; nothing about it belongs in this process.
	pub fn open_link(&self, url: &QString) {
		let url = String::from(url);
		if !url.starts_with("http://") && !url.starts_with("https://") {
			return;
		}
		let _ = std::process::Command::new("xdg-open")
			.arg(&url)
			.stdin(std::process::Stdio::null())
			.stdout(std::process::Stdio::null())
			.stderr(std::process::Stdio::null())
			.spawn();
	}

	pub fn config_choice_count(&self) -> i32 {
		count(self.question().map_or(0, |question| question.choices.len()))
	}

	pub fn config_choice_value(&self, row: i32) -> QString {
		self.choice(row).map_or_else(QString::default, |choice| choice.value.clone().into())
	}

	pub fn config_choice_help(&self, row: i32) -> QString {
		self.choice(row).map_or_else(QString::default, |choice| choice.help.clone().into())
	}

	pub fn remote_count(&self) -> i32 { count(self.remotes().map_or(0, Remotes::len)) }

	pub fn remote_name(&self, row: i32) -> QString {
		self.remote(row).map_or_else(QString::default, |remote| remote.name.clone().into())
	}

	/// The location string that navigates into a remote's root.
	pub fn remote_location(&self, row: i32) -> QString {
		self.remote(row).map_or_else(QString::default, |remote| format!("{}:/", remote.name).into())
	}

	pub fn task_count(&self) -> i32 { count(self.rust().snapshot.tasks.len()) }

	pub fn task_id(&self, row: i32) -> u64 { self.task(row).map_or(0, |task| task.id.0) }

	pub fn task_title(&self, row: i32) -> QString {
		self.task(row).map_or_else(QString::default, |task| task.title.clone().into())
	}

	pub fn task_status(&self, row: i32) -> QString {
		self.task(row).map_or_else(QString::default, |task| status_text(task.status).into())
	}

	pub fn task_progress(&self, row: i32) -> i32 {
		self.task(row).map_or(0, |task| i32::from(task.progress.basis_points()))
	}

	pub fn task_active(&self, row: i32) -> bool { self.task(row).is_some_and(TaskInfo::can_cancel) }

	pub fn cancel_task(self: Pin<&mut Self>, id: u64) {
		self.submit(Command::CancelTask(thallium_protocol::TaskId(id)));
	}

	/// Read a location written by the frontend, which may name a remote.
	fn location(&self, path: &QString) -> Location {
		let text = String::from(path);
		match self.remotes() {
			Some(remotes) => remotes.parse_display(&text),
			None => Location::local(NativePath::new(text.into_bytes())).normalize(),
		}
	}

	/// Render a location the way [`Self::location`] reads it back.
	fn display(&self, location: Option<&thallium_protocol::LocationDto>) -> String {
		let Some(location) = location else { return String::new() };
		match self.remotes() {
			Some(remotes) => remotes.display(&location.to_location()),
			None => location.path.display_lossy.clone(),
		}
	}

	/// Send a command without waiting for it to finish.
	///
	/// Used for navigation, whose effects include reading the directory: on a
	/// remote that is a network round trip, and blocking on it froze the window
	/// for its whole duration. The interface shows itself as loading instead.
	fn submit_detached(mut self: Pin<&mut Self>, command: Command) {
		let started = self.as_mut().rust_mut().backend.as_mut().map_or_else(
			|| Err(crate::BackendError::NotStarted),
			|backend| backend.submit_detached(command),
		);
		match started {
			Ok(pending) => {
				self.as_mut().rust_mut().pending = Some(pending);
				self.as_mut().set_loading(true);
				self.as_mut().set_last_error(QString::default());
			}
			Err(error) => self.as_mut().set_last_error(error.to_string().into()),
		}
	}

	/// Collect an outstanding navigation, if it has finished.
	fn collect_pending(mut self: Pin<&mut Self>) -> bool {
		let outcome = {
			let mut rust = self.as_mut().rust_mut();
			let Some(pending) = rust.pending.as_mut() else { return false };
			pending.try_recv()
		};
		match outcome {
			Err(tokio::sync::oneshot::error::TryRecvError::Empty) => false,
			outcome => {
				self.as_mut().rust_mut().pending = None;
				self.as_mut().set_loading(false);
				if let Ok(Err(message)) = outcome {
					self.as_mut().set_last_error(message.into());
				}
				true
			}
		}
	}

	fn submit(mut self: Pin<&mut Self>, command: Command) {
		let result = self
			.as_mut()
			.rust_mut()
			.backend
			.as_mut()
			.map_or_else(|| Err(crate::BackendError::NotStarted), |backend| backend.submit(command));
		match result {
			Ok(()) => self.as_mut().set_last_error(QString::default()),
			Err(error) => self.as_mut().set_last_error(error.to_string().into()),
		}
		self.reload();
	}

	fn reload(mut self: Pin<&mut Self>) {
		let result = self
			.as_mut()
			.rust_mut()
			.backend
			.as_ref()
			.map_or_else(|| Err(crate::BackendError::NotStarted), Backend::snapshot);
		match result {
			Ok(snapshot) if snapshot != self.rust().snapshot => {
				let path = self.display(snapshot.current_location.as_ref());
				self.as_mut().rust_mut().snapshot = snapshot;
				self.as_mut().set_current_path(path.into());
				let revision = self.model_revision().saturating_add(1);
				self.as_mut().set_model_revision(revision);
			}
			Ok(_) => {}
			Err(error) => self.as_mut().set_last_error(error.to_string().into()),
		}
	}

	fn current(&self) -> Option<Location> {
		self.rust().snapshot.current_location.as_ref().map(|location| location.to_location())
	}

	fn row(&self, row: i32) -> Option<&DirectoryEntryRow> {
		usize::try_from(row).ok().and_then(|row| self.rust().snapshot.directory.entries.get(row))
	}

	/// Record where a configuration conversation now stands.
	///
	/// A finished conversation re-reads the configuration and bumps the model
	/// revision, so the new remote appears in the sidebar without a restart.
	fn take_up(mut self: Pin<&mut Self>, outcome: Option<BackendResult<Session>>) {
		match outcome {
			Some(Ok(session)) => {
				let step = session.step().clone();
				self.as_mut().rust_mut().session = Some(session);
				match step {
					Step::Ask(_) => {
						self.as_mut().set_last_error(QString::default());
						self.as_mut().begin_consent();
					}
					Step::Failed(message) => {
						self.as_mut().rust_mut().session = None;
						self.as_mut().rust_mut().consent = None;
						self.as_mut().set_last_error(message.into());
					}
					Step::Done => {
						self.as_mut().rust_mut().session = None;
						self.as_mut().rust_mut().consent = None;
						self.as_mut().set_last_error(QString::default());
						if let Some(backend) = self.as_mut().rust_mut().backend.as_ref()
							&& let Err(error) = backend.refresh_remotes()
						{
							self.as_mut().set_last_error(error.to_string().into());
						}
						let revision = self.model_revision().saturating_add(1);
						self.as_mut().set_model_revision(revision);
					}
				}
			}
			Some(Err(error)) => {
				self.as_mut().rust_mut().session = None;
				self.as_mut().set_last_error(error.to_string().into());
			}
			None => {}
		}
	}

	fn question(&self) -> Option<&thallium_rclone::config::Question> {
		match self.rust().session.as_ref()?.step() {
			Step::Ask(question) => Some(question),
			Step::Done | Step::Failed(_) => None,
		}
	}

	fn choice(&self, row: i32) -> Option<&thallium_rclone::config::Choice> {
		let question = self.question()?;
		usize::try_from(row).ok().and_then(|row| question.choices.get(row))
	}

	fn provider(&self, row: i32) -> Option<&Provider> {
		usize::try_from(row).ok().and_then(|row| self.rust().providers.get(row))
	}

	fn remotes(&self) -> Option<&Remotes> { self.rust().backend.as_ref().map(Backend::remotes) }

	fn remote(&self, row: i32) -> Option<thallium_rclone::Remote> {
		usize::try_from(row).ok().and_then(|row| self.remotes()?.at(row))
	}

	fn task(&self, row: i32) -> Option<&TaskInfo> {
		usize::try_from(row).ok().and_then(|row| self.rust().snapshot.tasks.get(row))
	}

	fn selected_locations(&self) -> Vec<Location> {
		let selected: BTreeSet<EntryId> =
			self.rust().snapshot.selection.iter().map(|id| id.to_entry_id()).collect();
		self
			.rust()
			.snapshot
			.directory
			.entries
			.iter()
			.filter(|entry| selected.contains(&entry.id.to_entry_id()))
			.map(|entry| entry.location.to_location())
			.collect()
	}
}

/// The files in the XDG templates folder, which "New file" offers as starting
/// points. Read on demand: the folder is small and edited outside the app.
fn templates() -> Vec<PathBuf> {
	let Some(folder) = templates_folder() else { return Vec::new() };
	let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
		.map(|entries| {
			entries
				.filter_map(Result::ok)
				.filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
				.map(|entry| entry.path())
				.collect()
		})
		.unwrap_or_default();
	found.sort();
	found
}

fn template(row: i32) -> Option<PathBuf> {
	usize::try_from(row).ok().and_then(|row| templates().into_iter().nth(row))
}

/// `XDG_TEMPLATES_DIR` from `user-dirs.dirs`, or `~/Templates`. A value equal
/// to the home folder means templates are switched off.
fn templates_folder() -> Option<PathBuf> {
	let home = PathBuf::from(std::env::var_os("HOME")?);
	let config = std::env::var_os("XDG_CONFIG_HOME")
		.map_or_else(|| home.join(".config"), PathBuf::from)
		.join("user-dirs.dirs");
	let configured = std::fs::read_to_string(config).ok().and_then(|text| {
		text.lines().find_map(|line| {
			let value = line.trim().strip_prefix("XDG_TEMPLATES_DIR=")?.trim_matches('"');
			Some(match value.strip_prefix("$HOME") {
				Some(rest) => home.join(rest.trim_start_matches('/')),
				None => PathBuf::from(value),
			})
		})
	});
	let folder = configured.unwrap_or_else(|| home.join("Templates"));
	(folder != home).then_some(folder)
}

/// The entry's exact filesystem bytes, which the icon layer needs: a name that
/// is not valid UTF-8 still has to hash to the right thumbnail.
fn native_path(path: &PathBytes) -> PathBuf { PathBuf::from(OsStr::from_bytes(&path.bytes)) }

fn count(length: usize) -> i32 { i32::try_from(length).unwrap_or(i32::MAX) }

fn format_size(bytes: u64) -> String {
	const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
	let mut value = bytes as f64;
	let mut unit = 0;
	while value >= 1024.0 && unit < UNITS.len() - 1 {
		value /= 1024.0;
		unit += 1;
	}
	if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", UNITS[unit]) }
}

fn status_text(status: TaskStatus) -> &'static str {
	match status {
		TaskStatus::Pending => "Pending",
		TaskStatus::Started => "Running",
		TaskStatus::Cancelling => "Cancelling",
		TaskStatus::Succeeded => "Complete",
		TaskStatus::Failed => "Failed",
		TaskStatus::Cancelled => "Cancelled",
	}
}

#[cfg(test)]
mod tests {
	use thallium_protocol::TaskStatus;

	use super::{count, format_size, status_text};

	#[test]
	fn presentation_helpers_are_stable() {
		assert_eq!(count(3), 3);
		assert_eq!(format_size(1536), "1.5 KiB");
		assert_eq!(status_text(TaskStatus::Succeeded), "Complete");
	}
}
