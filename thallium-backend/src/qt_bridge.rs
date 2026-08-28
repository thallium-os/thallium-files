//! Thin CXX-Qt adapter over owned snapshots from [`crate::Backend`].

use core::pin::Pin;
use std::collections::BTreeSet;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use thallium_actions::Command;
use thallium_fs_core::{ConflictPolicy, EntryId, Location, NativePath};
use thallium_protocol::{BrowserStateSnapshot, DirectoryEntryRow, TaskInfo, TaskStatus};

use crate::{Backend, BackendConfig};

#[cxx_qt::bridge]
pub mod ffi {
	unsafe extern "C++" {
		include!("cxx-qt-lib/qstring.h");
		type QString = cxx_qt_lib::QString;
	}

	extern "RustQt" {
		#[qobject]
		#[qml_element]
		#[qproperty(QString, current_path, cxx_name = "currentPath")]
		#[qproperty(QString, home_path, cxx_name = "homePath")]
		#[qproperty(QString, last_error, cxx_name = "lastError")]
		#[qproperty(u64, model_revision, cxx_name = "modelRevision")]
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
	backend:        Option<Backend>,
	snapshot:       BrowserStateSnapshot,
}

impl Default for BackendBridgeRust {
	fn default() -> Self {
		match Backend::start(BackendConfig::default()) {
			Ok(backend) => {
				let snapshot = backend.snapshot().unwrap_or_default();
				let current_path = display_location(snapshot.current_location.as_ref());
				Self {
					current_path: current_path.clone().into(),
					home_path: current_path.into(),
					last_error: QString::default(),
					model_revision: snapshot.revision.0,
					backend: Some(backend),
					snapshot,
				}
			}
			Err(error) => Self {
				current_path:   QString::default(),
				home_path:      QString::default(),
				last_error:     error.to_string().into(),
				model_revision: 0,
				backend:        None,
				snapshot:       BrowserStateSnapshot::default(),
			},
		}
	}
}

impl ffi::BackendBridge {
	pub fn poll(mut self: Pin<&mut Self>) {
		if let Some(backend) = self.as_mut().rust_mut().backend.as_mut() {
			while backend.next_event().is_ok_and(|event| event.is_some()) {}
		}
		self.reload();
	}

	pub fn navigate(self: Pin<&mut Self>, path: &QString) {
		self.submit(Command::Navigate(location(path)));
	}

	pub fn navigate_row(self: Pin<&mut Self>, row: i32) {
		let location =
			self.row(row).filter(|entry| entry.is_directory()).map(|entry| entry.location.to_location());
		if let Some(location) = location {
			self.submit(Command::Navigate(location));
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
			self.submit(Command::Navigate(parent));
		}
	}

	pub fn back(self: Pin<&mut Self>) { self.submit(Command::NavigateBack); }

	pub fn forward(self: Pin<&mut Self>) { self.submit(Command::NavigateForward); }

	pub fn refresh(self: Pin<&mut Self>) { self.submit(Command::Refresh); }

	pub fn entry_count(&self) -> i32 { count(self.rust().snapshot.directory.entries.len()) }

	pub fn entry_name(&self, row: i32) -> QString {
		self.row(row).map_or_else(QString::default, |entry| entry.name.display_lossy.clone().into())
	}

	pub fn entry_path(&self, row: i32) -> QString {
		self
			.row(row)
			.map_or_else(QString::default, |entry| entry.location.path.display_lossy.clone().into())
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

	pub fn copy_selected(self: Pin<&mut Self>, destination: &QString) {
		let sources = self.selected_locations();
		self.submit(Command::Copy {
			sources,
			destination: location(destination),
			conflict_policy: ConflictPolicy::Rename,
		});
	}

	pub fn move_selected(self: Pin<&mut Self>, destination: &QString) {
		let sources = self.selected_locations();
		self.submit(Command::Move {
			sources,
			destination: location(destination),
			conflict_policy: ConflictPolicy::Rename,
		});
	}

	pub fn delete_selected(self: Pin<&mut Self>) {
		let targets = self.selected_locations();
		self.submit(Command::Delete { targets });
	}

	pub fn trash_selected(self: Pin<&mut Self>) {
		let targets = self.selected_locations();
		self.submit(Command::Trash { targets });
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
				let path = display_location(snapshot.current_location.as_ref());
				self.as_mut().rust_mut().snapshot = snapshot;
				self.as_mut().set_current_path(path.into());
				let revision = self.model_revision().saturating_add(1);
				self.as_mut().set_model_revision(revision);
			}
			Ok(_) => {}
			Err(error) => self.as_mut().set_last_error(error.to_string().into()),
		}
	}

	fn row(&self, row: i32) -> Option<&DirectoryEntryRow> {
		usize::try_from(row).ok().and_then(|row| self.rust().snapshot.directory.entries.get(row))
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

fn location(path: &QString) -> Location {
	Location::local(NativePath::new(String::from(path).into_bytes())).normalize()
}

fn display_location(location: Option<&thallium_protocol::LocationDto>) -> String {
	location.map_or_else(String::new, |location| location.path.display_lossy.clone())
}

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
