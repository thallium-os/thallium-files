//! Aggregate task registry record and immutable admission specification.

use std::{sync::{Arc, atomic::{AtomicU8, Ordering}}, time::{SystemTime, UNIX_EPOCH}};

use thallium_fs_core::{FsError, Location};
use thallium_protocol::{LocationDto, TaskId, TaskInfo, TaskIssue, TaskKind, TaskRevision};
use tokio::sync::{Notify, watch};
use tokio_util::sync::CancellationToken;

use crate::{AggregateProgress, TaskHandle, TaskInput, TaskOutcome, TaskStatus};

/// Immutable facts admitted for one semantic user command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskSpec {
	pub id:    TaskId,
	pub title: String,
	pub kind:  TaskKind,
	pub input: TaskInput,
}

/// Mutable registry-owned state for one aggregate task.
pub struct Task {
	pub(crate) spec:         TaskSpec,
	pub(crate) revision:     TaskRevision,
	pub(crate) status:       TaskStatus,
	pub(crate) progress:     AggregateProgress,
	pub(crate) issue:        Option<FsError>,
	pub(crate) current_item: Option<Location>,
	pub(crate) created_at:   SystemTime,
	pub(crate) updated_at:   SystemTime,
	pub(crate) cancellation: CancellationToken,
	pub(crate) snapshots:    watch::Sender<TaskInfo>,
	pub(crate) lifecycle:    Arc<AtomicU8>,
	pub(crate) completion:   Arc<Notify>,
}

impl Task {
	pub(crate) fn new(spec: TaskSpec) -> (Self, TaskHandle) {
		let cancellation = CancellationToken::new();
		let lifecycle = Arc::new(AtomicU8::new(TaskStatus::Queued.atomic_value()));
		let completion = Arc::new(Notify::new());
		let now = SystemTime::now();
		let initial = TaskInfo {
			id:            spec.id,
			revision:      TaskRevision(0),
			kind:          spec.kind,
			status:        thallium_protocol::TaskStatus::Pending,
			title:         spec.title.clone(),
			progress:      Default::default(),
			current_item:  None,
			issue:         None,
			created_at_ms: millis(now),
			updated_at_ms: millis(now),
		};
		let (snapshots, receiver) = watch::channel(initial);
		let task = Self {
			spec,
			revision: TaskRevision(0),
			status: TaskStatus::Queued,
			progress: AggregateProgress::default(),
			issue: None,
			current_item: None,
			created_at: now,
			updated_at: now,
			cancellation: cancellation.clone(),
			snapshots,
			lifecycle: Arc::clone(&lifecycle),
			completion: Arc::clone(&completion),
		};
		let handle =
			TaskHandle { id: task.spec.id, cancellation, snapshots: receiver, lifecycle, completion };
		(task, handle)
	}

	/// Return the task's stable public identity.
	#[must_use]
	pub fn id(&self) -> TaskId { self.spec.id }

	/// Build an owned, boundary-safe observation of the current record.
	#[must_use]
	pub fn snapshot(&self) -> TaskInfo {
		TaskInfo {
			id:            self.id(),
			revision:      self.revision,
			kind:          self.spec.kind,
			status:        protocol_status(self.status),
			title:         self.spec.title.clone(),
			progress:      self.progress.protocol(),
			current_item:  self.current_item.as_ref().map(LocationDto::from),
			issue:         self.issue.as_ref().map(|error| TaskIssue {
				code:     error.code,
				message:  error.message.clone(),
				location: error.location.as_ref().map(LocationDto::from),
				os_code:  error.os_code,
			}),
			created_at_ms: millis(self.created_at),
			updated_at_ms: millis(self.updated_at),
		}
	}

	pub(crate) fn publish(&mut self) -> TaskInfo {
		self.revision = TaskRevision(self.revision.0.saturating_add(1));
		self.updated_at = SystemTime::now();
		self.lifecycle.store(self.status.atomic_value(), Ordering::Release);
		let snapshot = self.snapshot();
		self.snapshots.send_replace(snapshot.clone());
		if self.status.is_terminal() {
			self.completion.notify_waiters();
		}
		snapshot
	}

	pub(crate) fn finish(&mut self, result: Result<(), FsError>) -> TaskInfo {
		self.current_item = None;
		match result {
			Ok(()) => {
				self.progress.items_completed = self.progress.items_discovered;
				self.progress.bytes_completed = self.progress.bytes_discovered;
				self.status = TaskStatus::Finished(TaskOutcome::Succeeded);
			}
			Err(error) if error.code == thallium_fs_core::FsErrorCode::Cancelled => {
				self.issue = Some(error);
				self.status = TaskStatus::Finished(TaskOutcome::Cancelled);
			}
			Err(error) => {
				self.issue = Some(error);
				self.status = TaskStatus::Finished(TaskOutcome::Failed);
			}
		}
		self.publish()
	}
}

fn protocol_status(status: TaskStatus) -> thallium_protocol::TaskStatus {
	match status {
		TaskStatus::Queued => thallium_protocol::TaskStatus::Pending,
		TaskStatus::Discovering | TaskStatus::Running | TaskStatus::WaitingForDecision { .. } => {
			thallium_protocol::TaskStatus::Started
		}
		TaskStatus::Cancelling => thallium_protocol::TaskStatus::Cancelling,
		TaskStatus::Finished(TaskOutcome::Succeeded) => thallium_protocol::TaskStatus::Succeeded,
		TaskStatus::Finished(TaskOutcome::CompletedWithErrors | TaskOutcome::Failed) => {
			thallium_protocol::TaskStatus::Failed
		}
		TaskStatus::Finished(TaskOutcome::Cancelled) => thallium_protocol::TaskStatus::Cancelled,
	}
}

fn millis(time: SystemTime) -> u64 {
	time.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().min(u128::from(u64::MAX)) as u64
}
