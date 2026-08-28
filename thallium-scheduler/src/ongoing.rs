//! Stable-order, single-owner aggregate task registry.

use std::collections::{HashMap, VecDeque};

use thallium_protocol::{TaskId, TaskInfo};

use crate::{ActiveWork, CleanupOutcome, SchedulerError, SchedulerResult, Task, TaskOutcome, TaskStatus, WorkPhase, WorkerEvent};

/// Registry owned exclusively by the scheduler reducer.
#[derive(Default)]
pub struct Ongoing {
	tasks: HashMap<TaskId, Task>,
	order: VecDeque<TaskId>,
}

impl Ongoing {
	#[must_use]
	pub fn new() -> Self { Self::default() }

	#[must_use]
	pub fn len(&self) -> usize { self.tasks.len() }

	#[must_use]
	pub fn is_empty(&self) -> bool { self.tasks.is_empty() }

	/// Insert a task at the end of the stable presentation order.
	pub fn add(&mut self, task: Task) -> SchedulerResult<()> {
		let id = task.id();
		if self.tasks.contains_key(&id) {
			return Err(SchedulerError::InvalidInput(format!("task {} already exists", id.0)));
		}
		self.order.push_back(id);
		self.tasks.insert(id, task);
		Ok(())
	}

	/// Reduce a typed worker fact and immediately publish its new revision.
	pub fn reduce(&mut self, event: WorkerEvent) -> SchedulerResult<TaskInfo> {
		let id = event.task_id();
		let task = self.tasks.get_mut(&id).ok_or(SchedulerError::TaskNotFound(id))?;
		match event {
			WorkerEvent::WorkDiscovered { location, bytes, .. } => {
				task.progress.items_discovered = task.progress.items_discovered.saturating_add(1);
				task.progress.bytes_discovered = task.progress.bytes_discovered.saturating_add(bytes);
				task.current_item = Some(location);
				task.status = TaskStatus::Discovering;
			}
			WorkerEvent::ItemStarted { location, .. } => {
				task.current_item = Some(location.clone());
				task.progress.active_items = vec![ActiveWork { location, phase: WorkPhase::Reading }];
				task.status = TaskStatus::Running;
			}
			WorkerEvent::BytesAdvanced { bytes, .. } => {
				task.progress.bytes_completed = task.progress.bytes_completed.saturating_add(bytes);
			}
			WorkerEvent::ItemSucceeded { .. } => {
				task.progress.items_completed = task.progress.items_completed.saturating_add(1);
				task.progress.active_items.clear();
			}
			WorkerEvent::ItemFailed { error, .. } => {
				task.progress.items_failed = task.progress.items_failed.saturating_add(1);
				task.progress.active_items.clear();
				task.issue = Some(error);
				task.status = TaskStatus::Finished(TaskOutcome::Failed);
			}
			WorkerEvent::ItemSkipped { .. } => {
				task.progress.items_skipped = task.progress.items_skipped.saturating_add(1);
				task.progress.items_completed = task.progress.items_completed.saturating_add(1);
				task.progress.active_items.clear();
			}
			WorkerEvent::DiscoveryFinished { .. } => task.progress.discovery_complete = true,
			WorkerEvent::DiscoveryFailed { error, .. } => {
				task.issue = Some(error);
				task.status = TaskStatus::Finished(TaskOutcome::Failed);
			}
			WorkerEvent::CleanupFinished { outcome, .. } => match outcome {
				CleanupOutcome::Succeeded => {
					task.status = TaskStatus::Finished(TaskOutcome::Cancelled);
				}
				CleanupOutcome::Failed(error) => {
					task.issue = Some(error);
					task.status = TaskStatus::Finished(TaskOutcome::Failed);
				}
			},
		}
		Ok(task.publish())
	}

	#[must_use]
	pub fn snapshot(&self, id: TaskId) -> Option<TaskInfo> { self.tasks.get(&id).map(Task::snapshot) }

	/// Return snapshots in insertion order, including terminal history.
	#[must_use]
	pub fn snapshots(&self) -> Vec<TaskInfo> {
		self.order.iter().filter_map(|id| self.snapshot(*id)).collect()
	}

	/// Remove a terminal task while preserving the order of all others.
	pub fn dismiss(&mut self, id: TaskId) -> SchedulerResult<TaskInfo> {
		let snapshot = self.snapshot(id).ok_or(SchedulerError::TaskNotFound(id))?;
		if !snapshot.status.is_terminal() {
			return Err(SchedulerError::InvalidInput("only terminal tasks may be dismissed".into()));
		}
		self.tasks.remove(&id);
		self.order.retain(|candidate| *candidate != id);
		Ok(snapshot)
	}

	pub(crate) fn get_mut(&mut self, id: TaskId) -> Option<&mut Task> { self.tasks.get_mut(&id) }

	pub(crate) fn trim_terminal(&mut self, maximum: usize) -> Vec<TaskId> {
		let mut terminals = self
			.order
			.iter()
			.filter(|id| self.tasks.get(id).is_some_and(|task| task.status.is_terminal()))
			.count();
		let mut removed = Vec::new();
		if terminals <= maximum {
			return removed;
		}
		let ids: Vec<_> = self.order.iter().copied().collect();
		for id in ids {
			if terminals <= maximum {
				break;
			}
			if self.tasks.get(&id).is_some_and(|task| task.status.is_terminal()) {
				self.tasks.remove(&id);
				self.order.retain(|candidate| *candidate != id);
				removed.push(id);
				terminals -= 1;
			}
		}
		removed
	}
}

#[cfg(test)]
mod tests {
	use thallium_fs_core::{Location, NativePath};
	use thallium_protocol::{TaskId, TaskKind};

	use super::Ongoing;
	use crate::{DeleteTask, Priority, Task, TaskInput, TaskSpec};

	fn task(id: u64) -> Task {
		Task::new(TaskSpec {
			id:    TaskId(id),
			title: format!("task {id}"),
			kind:  TaskKind::Delete,
			input: TaskInput::Delete(DeleteTask {
				targets:  vec![Location::local(NativePath::from(&b"/tmp/item"[..]))],
				priority: Priority::Normal,
			}),
		})
		.0
	}

	#[test]
	fn snapshots_preserve_insertion_order() {
		let mut registry = Ongoing::new();
		registry.add(task(7)).expect("add first");
		registry.add(task(2)).expect("add second");
		registry.add(task(9)).expect("add third");

		let ids: Vec<_> = registry.snapshots().into_iter().map(|task| task.id.0).collect();
		assert_eq!(ids, [7, 2, 9]);
	}
}
