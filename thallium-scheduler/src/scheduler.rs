//! Bounded FIFO task scheduler and cancellable filesystem workers.

use std::{collections::{HashSet, VecDeque}, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}, time::{Duration, Instant}};

use thallium_fs_core::{ConflictPolicy, FileAttrs, FileKind, FsError, FsErrorCode, Location, NativePath};
use thallium_protocol::{TaskId, TaskInfo, TaskKind};
use thallium_vfs::{DeleteOptions, FileSystem, OpenWriteOptions, StatOptions};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, sync::{mpsc, oneshot}};
use tokio_util::sync::CancellationToken;

use crate::{CopyTask, DeleteTask, MoveTask, Ongoing, SchedulerError, SchedulerEvent, SchedulerResult, Task, TaskHandle, TaskInput, TaskSpec, TaskStatus, TrashTask};

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);
const COPY_BUFFER_SIZE: usize = 64 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(50);

/// Fixed v1 bounds for admission, execution, and retained observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerConfig {
	/// Maximum aggregate mutation tasks dispatched concurrently.
	pub active_mutation_tasks:   usize,
	/// Reserved for the later leaf-worker split; v1 executes leaves serially per
	/// aggregate task.
	pub leaf_file_io_workers:    usize,
	/// Reserved for a later independently limited metadata pool.
	pub metadata_workers:        usize,
	pub command_capacity:        usize,
	pub worker_event_capacity:   usize,
	pub per_task_work_capacity:  usize,
	pub retained_terminal_tasks: usize,
}

impl Default for SchedulerConfig {
	fn default() -> Self {
		Self {
			active_mutation_tasks:   2,
			leaf_file_io_workers:    2,
			metadata_workers:        2,
			command_capacity:        64,
			worker_event_capacity:   256,
			per_task_work_capacity:  4_096,
			retained_terminal_tasks: 64,
		}
	}
}

/// Renderer-neutral scheduler façade.
pub struct Scheduler<V: FileSystem = thallium_vfs::LocalBackend> {
	command_tx: mpsc::Sender<Command>,
	event_rx:   tokio::sync::Mutex<mpsc::Receiver<SchedulerEvent>>,
	event_sync: Arc<EventSync>,
	_marker:    std::marker::PhantomData<fn() -> V>,
}

struct EventSync {
	dirty:  AtomicBool,
	notify: tokio::sync::Notify,
}

#[derive(Clone)]
struct EventPort {
	tx:   mpsc::Sender<SchedulerEvent>,
	sync: Arc<EventSync>,
}

impl EventPort {
	fn publish(&self, event: SchedulerEvent) {
		if self.tx.try_send(event).is_err() {
			self.sync.dirty.store(true, Ordering::Release);
			self.sync.notify.notify_one();
		}
	}
}

enum Command {
	Submit(TaskInput, oneshot::Sender<SchedulerResult<TaskHandle>>),
	Cancel(TaskId, oneshot::Sender<SchedulerResult<()>>),
	Dismiss(TaskId, oneshot::Sender<SchedulerResult<()>>),
	Snapshot(TaskId, oneshot::Sender<SchedulerResult<TaskInfo>>),
	Snapshots(oneshot::Sender<SchedulerResult<Vec<TaskInfo>>>),
	Shutdown(oneshot::Sender<SchedulerResult<()>>),
}

enum WorkerUpdate {
	Prepared { id: TaskId, items: u64, bytes: u64 },
	ItemStarted { id: TaskId, location: Location },
	Bytes { id: TaskId, bytes: u64 },
	ItemCompleted { id: TaskId, accounted_bytes: u64 },
	Finished { id: TaskId, result: Result<(), FsError> },
}

impl<V: FileSystem + 'static> Scheduler<V> {
	/// Construct a scheduler. This must run inside the backend-owned Tokio
	/// runtime because it starts the private reducer task immediately.
	#[must_use]
	pub fn new(vfs: Arc<V>, config: SchedulerConfig) -> Self {
		let (command_tx, command_rx) = mpsc::channel(config.command_capacity.max(1));
		let (event_tx, event_rx) = mpsc::channel(config.worker_event_capacity.max(1));
		let (worker_tx, worker_rx) = mpsc::channel(config.worker_event_capacity.max(1));
		let event_sync =
			Arc::new(EventSync { dirty: AtomicBool::new(false), notify: tokio::sync::Notify::new() });
		let event_port = EventPort { tx: event_tx, sync: Arc::clone(&event_sync) };
		tokio::spawn(run_manager(vfs, config, command_rx, worker_tx, worker_rx, event_port));
		Self {
			command_tx,
			event_rx: tokio::sync::Mutex::new(event_rx),
			event_sync,
			_marker: std::marker::PhantomData,
		}
	}

	/// Admit one semantic task. Admission is bounded and does not wait for queue
	/// capacity, so an overloaded caller receives [`SchedulerError::QueueFull`].
	pub async fn add(&self, input: TaskInput) -> SchedulerResult<TaskHandle> {
		let (reply, response) = oneshot::channel();
		self.try_command(Command::Submit(input, reply))?;
		response.await.map_err(|_| SchedulerError::Unavailable)?
	}

	/// Request cooperative cancellation by stable identity.
	pub async fn cancel(&self, id: TaskId) -> SchedulerResult<()> {
		let (reply, response) = oneshot::channel();
		self.try_command(Command::Cancel(id, reply))?;
		response.await.map_err(|_| SchedulerError::Unavailable)?
	}

	/// Remove one retained terminal task.
	pub async fn dismiss(&self, id: TaskId) -> SchedulerResult<()> {
		let (reply, response) = oneshot::channel();
		self.try_command(Command::Dismiss(id, reply))?;
		response.await.map_err(|_| SchedulerError::Unavailable)?
	}

	/// Obtain one owned boundary-safe task snapshot.
	pub async fn snapshot(&self, id: TaskId) -> SchedulerResult<TaskInfo> {
		let (reply, response) = oneshot::channel();
		self.try_command(Command::Snapshot(id, reply))?;
		response.await.map_err(|_| SchedulerError::Unavailable)?
	}

	/// Obtain the full retained task set in stable insertion order.
	pub async fn snapshots(&self) -> SchedulerResult<Vec<TaskInfo>> {
		let (reply, response) = oneshot::channel();
		self.try_command(Command::Snapshots(reply))?;
		response.await.map_err(|_| SchedulerError::Unavailable)?
	}

	/// Receive the next coalescible scheduler observation.
	pub async fn next_event(&self) -> SchedulerResult<SchedulerEvent> {
		let mut events = self.event_rx.lock().await;
		loop {
			if self.event_sync.dirty.swap(false, Ordering::AcqRel) {
				while events.try_recv().is_ok() {}
				return Ok(SchedulerEvent::ResyncRequired);
			}
			let notified = self.event_sync.notify.notified();
			if self.event_sync.dirty.load(Ordering::Acquire) {
				continue;
			}
			tokio::select! {
				biased;
				() = notified => continue,
				event = events.recv() => return event.ok_or(SchedulerError::Unavailable),
			}
		}
	}

	/// Stop admission, cooperatively cancel active work, clean staging files,
	/// and wait for worker completion.
	pub async fn shutdown(&self) -> SchedulerResult<()> {
		let (reply, response) = oneshot::channel();
		self.try_command(Command::Shutdown(reply))?;
		response.await.map_err(|_| SchedulerError::Unavailable)?
	}

	fn try_command(&self, command: Command) -> SchedulerResult<()> {
		self.command_tx.try_send(command).map_err(|error| match error {
			mpsc::error::TrySendError::Full(_) => SchedulerError::QueueFull,
			mpsc::error::TrySendError::Closed(_) => SchedulerError::Unavailable,
		})
	}
}

async fn run_manager<V: FileSystem + 'static>(
	vfs: Arc<V>,
	config: SchedulerConfig,
	mut commands: mpsc::Receiver<Command>,
	worker_tx: mpsc::Sender<WorkerUpdate>,
	mut workers: mpsc::Receiver<WorkerUpdate>,
	events: EventPort,
) {
	let mut registry = Ongoing::new();
	let mut pending = VecDeque::new();
	let mut active = HashSet::new();
	let limit = config.active_mutation_tasks.max(1);
	let mut shutdown_reply: Option<oneshot::Sender<SchedulerResult<()>>> = None;
	let mut commands_open = true;

	loop {
		while active.len() < limit {
			let Some(id) = pending.pop_front() else { break };
			let Some(task) = registry.get_mut(id) else { continue };
			if task.cancellation.is_cancelled() {
				let snapshot = task.finish(Err(cancelled_error(None)));
				publish(&events, SchedulerEvent::Snapshot(Box::new(snapshot)));
				continue;
			}
			task.status = TaskStatus::Running;
			let snapshot = task.publish();
			publish(&events, SchedulerEvent::Snapshot(Box::new(snapshot)));
			let input = task.spec.input.clone();
			let cancellation = task.cancellation.clone();
			let tx = worker_tx.clone();
			let filesystem = Arc::clone(&vfs);
			active.insert(id);
			tokio::spawn(async move {
				let result = execute_task(&*filesystem, id, input, cancellation, &tx).await;
				let _ = tx.send(WorkerUpdate::Finished { id, result }).await;
			});
		}

		if shutdown_reply.is_some() && active.is_empty() && pending.is_empty() {
			if let Some(reply) = shutdown_reply.take() {
				let _ = reply.send(Ok(()));
			}
			break;
		}

		tokio::select! {
			command = commands.recv(), if commands_open => {
				match command {
					Some(Command::Shutdown(reply)) => {
						begin_shutdown(&mut registry, &mut pending, &active, &events);
						shutdown_reply = Some(reply);
						commands.close();
						commands_open = false;
					}
					Some(command) => handle_command(
						command,
						&mut registry,
						&mut pending,
						&active,
						&events,
						config.command_capacity.max(1),
						config.per_task_work_capacity.max(1),
					),
					None => {
						commands_open = false;
						begin_shutdown(&mut registry, &mut pending, &active, &events);
						if active.is_empty() {
							break;
						}
					}
				}
			}
			update = workers.recv() => {
				let Some(update) = update else { break };
				handle_worker_update(update, &mut registry, &mut active, &events, config.retained_terminal_tasks);
			}
		}
	}
}

fn handle_command(
	command: Command,
	registry: &mut Ongoing,
	pending: &mut VecDeque<TaskId>,
	active: &HashSet<TaskId>,
	events: &EventPort,
	pending_capacity: usize,
	per_task_capacity: usize,
) {
	match command {
		Command::Submit(input, reply) => {
			let result = admit(input, registry, pending, events, pending_capacity, per_task_capacity);
			let _ = reply.send(result);
		}
		Command::Cancel(id, reply) => {
			let result = cancel_task(id, registry, pending, active, events);
			let _ = reply.send(result);
		}
		Command::Dismiss(id, reply) => {
			let result = registry.dismiss(id).map(|_| publish(events, SchedulerEvent::Dismissed(id)));
			let _ = reply.send(result);
		}
		Command::Snapshot(id, reply) => {
			let result = registry.snapshot(id).ok_or(SchedulerError::TaskNotFound(id));
			let _ = reply.send(result);
		}
		Command::Snapshots(reply) => {
			let _ = reply.send(Ok(registry.snapshots()));
		}
		Command::Shutdown(_) => unreachable!("shutdown is handled by the manager loop"),
	}
}

fn admit(
	input: TaskInput,
	registry: &mut Ongoing,
	pending: &mut VecDeque<TaskId>,
	events: &EventPort,
	pending_capacity: usize,
	per_task_capacity: usize,
) -> SchedulerResult<TaskHandle> {
	input.validate().map_err(SchedulerError::InvalidInput)?;
	if pending.len() >= pending_capacity {
		return Err(SchedulerError::QueueFull);
	}
	let work_len = match &input {
		TaskInput::Copy(task) => task.sources.len(),
		TaskInput::Move(task) => task.sources.len(),
		TaskInput::Delete(task) => task.targets.len(),
		TaskInput::Trash(task) => task.targets.len(),
	};
	if work_len > per_task_capacity {
		return Err(SchedulerError::InvalidInput(format!(
			"task contains {work_len} items, exceeding the configured limit of {per_task_capacity}"
		)));
	}
	let id = TaskId(NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed));
	let (kind, title) = describe(&input);
	let (task, handle) = Task::new(TaskSpec { id, title, kind, input });
	let snapshot = task.snapshot();
	registry.add(task)?;
	pending.push_back(id);
	publish(events, SchedulerEvent::Snapshot(Box::new(snapshot)));
	Ok(handle)
}

fn cancel_task(
	id: TaskId,
	registry: &mut Ongoing,
	pending: &mut VecDeque<TaskId>,
	active: &HashSet<TaskId>,
	events: &EventPort,
) -> SchedulerResult<()> {
	let task = registry.get_mut(id).ok_or(SchedulerError::TaskNotFound(id))?;
	if task.status.is_terminal() {
		return Ok(());
	}
	task.cancellation.cancel();
	task.status = TaskStatus::Cancelling;
	let snapshot = task.publish();
	publish(events, SchedulerEvent::Snapshot(Box::new(snapshot)));
	if !active.contains(&id) {
		pending.retain(|candidate| *candidate != id);
		let snapshot = task.finish(Err(cancelled_error(None)));
		publish(events, SchedulerEvent::Snapshot(Box::new(snapshot)));
	}
	Ok(())
}

fn begin_shutdown(
	registry: &mut Ongoing,
	pending: &mut VecDeque<TaskId>,
	active: &HashSet<TaskId>,
	events: &EventPort,
) {
	for snapshot in registry.snapshots() {
		if !snapshot.status.is_terminal() {
			let _ = cancel_task(snapshot.id, registry, pending, active, events);
		}
	}
}

fn handle_worker_update(
	update: WorkerUpdate,
	registry: &mut Ongoing,
	active: &mut HashSet<TaskId>,
	events: &EventPort,
	retained_terminal_tasks: usize,
) {
	let id = match &update {
		WorkerUpdate::Prepared { id, .. }
		| WorkerUpdate::ItemStarted { id, .. }
		| WorkerUpdate::Bytes { id, .. }
		| WorkerUpdate::ItemCompleted { id, .. }
		| WorkerUpdate::Finished { id, .. } => *id,
	};
	let Some(task) = registry.get_mut(id) else { return };
	let snapshot = match update {
		WorkerUpdate::Prepared { items, bytes, .. } => {
			task.progress.items_discovered = items;
			task.progress.bytes_discovered = bytes;
			task.progress.discovery_complete = true;
			task.publish()
		}
		WorkerUpdate::ItemStarted { location, .. } => {
			task.current_item = Some(location);
			task.publish()
		}
		WorkerUpdate::Bytes { bytes, .. } => {
			task.progress.bytes_completed = task.progress.bytes_completed.saturating_add(bytes);
			task.publish()
		}
		WorkerUpdate::ItemCompleted { accounted_bytes, .. } => {
			task.progress.items_completed = task.progress.items_completed.saturating_add(1);
			task.progress.bytes_completed = task.progress.bytes_completed.saturating_add(accounted_bytes);
			task.publish()
		}
		WorkerUpdate::Finished { result, .. } => {
			active.remove(&id);
			task.finish(result)
		}
	};
	publish(events, SchedulerEvent::Snapshot(Box::new(snapshot)));
	for removed in registry.trim_terminal(retained_terminal_tasks) {
		publish(events, SchedulerEvent::Dismissed(removed));
	}
}

fn publish(events: &EventPort, event: SchedulerEvent) {
	// Snapshot delivery is coalescible, but loss is never silent: a full queue
	// sets a persistent dirty bit that forces the action owner to resynchronize.
	events.publish(event);
}

fn describe(input: &TaskInput) -> (TaskKind, String) {
	let (kind, verb, count) = match input {
		TaskInput::Copy(task) => (TaskKind::Copy, "Copy", task.sources.len()),
		TaskInput::Move(task) => (TaskKind::Move, "Move", task.sources.len()),
		TaskInput::Delete(task) => (TaskKind::Delete, "Delete", task.targets.len()),
		TaskInput::Trash(task) => (TaskKind::Trash, "Trash", task.targets.len()),
	};
	(kind, format!("{verb} {count} item{}", if count == 1 { "" } else { "s" }))
}

async fn execute_task<V: FileSystem>(
	vfs: &V,
	id: TaskId,
	input: TaskInput,
	cancellation: CancellationToken,
	updates: &mpsc::Sender<WorkerUpdate>,
) -> Result<(), FsError> {
	check_cancelled(&cancellation, None)?;
	let prepared = prepare(vfs, &input).await?;
	check_cancelled(&cancellation, None)?;
	let total_bytes = prepared.iter().map(|item| item.size).sum();
	send_update(updates, WorkerUpdate::Prepared {
		id,
		items: prepared.len() as u64,
		bytes: total_bytes,
	})
	.await?;
	let mut progress = ProgressReporter::new(id, updates);

	match input {
		TaskInput::Copy(task) => {
			execute_copy(vfs, &task, prepared, &cancellation, &mut progress).await?
		}
		TaskInput::Move(task) => {
			execute_move(vfs, &task, prepared, &cancellation, &mut progress).await?
		}
		TaskInput::Delete(task) => {
			execute_delete(vfs, &task, prepared, &cancellation, &mut progress).await?
		}
		TaskInput::Trash(task) => {
			execute_trash(vfs, &task, prepared, &cancellation, &mut progress).await?
		}
	}
	progress.flush().await?;
	Ok(())
}

struct PreparedItem {
	location: Location,
	size:     u64,
	attrs:    FileAttrs,
	kind:     FileKind,
}

async fn prepare<V: FileSystem>(vfs: &V, input: &TaskInput) -> Result<Vec<PreparedItem>, FsError> {
	let locations = match input {
		TaskInput::Copy(task) => &task.sources,
		TaskInput::Move(task) => &task.sources,
		TaskInput::Delete(task) => &task.targets,
		TaskInput::Trash(task) => &task.targets,
	};
	let mut prepared = Vec::with_capacity(locations.len());
	for location in locations {
		let follow_symlinks = matches!(input, TaskInput::Copy(_));
		let entry = vfs.stat(location, StatOptions { follow_symlinks }).await?;
		if matches!(input, TaskInput::Copy(_)) && entry.metadata.kind == FileKind::Directory {
			return Err(FsError::new(
				FsErrorCode::IsDirectory,
				"MVP copy supports regular files, not directory trees",
				Some(location.clone()),
			));
		}
		prepared.push(PreparedItem {
			location: location.clone(),
			size:     entry.metadata.size,
			attrs:    FileAttrs::from_metadata(&entry.metadata),
			kind:     entry.metadata.kind,
		});
	}
	Ok(prepared)
}

async fn execute_copy<V: FileSystem>(
	vfs: &V,
	task: &CopyTask,
	items: Vec<PreparedItem>,
	cancellation: &CancellationToken,
	progress: &mut ProgressReporter<'_>,
) -> Result<(), FsError> {
	for item in items {
		check_cancelled(cancellation, Some(item.location.clone()))?;
		progress.item_started(item.location.clone()).await?;
		let name = file_name(&item.location)?;
		let requested = task.destination.join(name);
		let outcome = copy_file(
			vfs,
			&item.location,
			&requested,
			task.options.conflict_policy,
			task.options.attributes.as_ref().unwrap_or(&item.attrs),
			cancellation,
			progress,
		)
		.await?;
		progress.item_completed(if outcome.copied { 0 } else { item.size }).await?;
	}
	Ok(())
}

async fn execute_move<V: FileSystem>(
	vfs: &V,
	task: &MoveTask,
	items: Vec<PreparedItem>,
	cancellation: &CancellationToken,
	progress: &mut ProgressReporter<'_>,
) -> Result<(), FsError> {
	for item in items {
		check_cancelled(cancellation, Some(item.location.clone()))?;
		progress.item_started(item.location.clone()).await?;
		let requested = task.destination.join(file_name(&item.location)?);
		let Some(destination) =
			resolve_destination(vfs, &requested, task.options.conflict_policy).await?
		else {
			progress.item_completed(item.size).await?;
			continue;
		};
		if destination == item.location {
			return Err(FsError::new(
				FsErrorCode::InvalidInput,
				"source and destination are the same entry",
				Some(destination),
			));
		}
		check_cancelled(cancellation, Some(item.location.clone()))?;
		match vfs.rename(&item.location, &destination).await {
			Ok(()) => progress.item_completed(item.size).await?,
			Err(error) if error.code == FsErrorCode::CrossDevice => {
				if item.kind == FileKind::Directory {
					return Err(FsError::new(
						FsErrorCode::Unsupported,
						"cross-filesystem directory moves are not supported in the MVP",
						Some(item.location),
					));
				}
				copy_file(
					vfs,
					&item.location,
					&destination,
					ConflictPolicy::Overwrite,
					task.options.attributes.as_ref().unwrap_or(&item.attrs),
					cancellation,
					progress,
				)
				.await?;
				// Publication is complete and valid. Finish the move even if
				// cancellation arrives here, avoiding a misleading half-move
				// with duplicate files.
				vfs.delete(&item.location, DeleteOptions { recursive: false }).await?;
				progress.item_completed(0).await?;
			}
			Err(error) => return Err(error),
		}
	}
	Ok(())
}

async fn execute_delete<V: FileSystem>(
	vfs: &V,
	_task: &DeleteTask,
	items: Vec<PreparedItem>,
	cancellation: &CancellationToken,
	progress: &mut ProgressReporter<'_>,
) -> Result<(), FsError> {
	for item in items {
		check_cancelled(cancellation, Some(item.location.clone()))?;
		progress.item_started(item.location.clone()).await?;
		vfs
			.delete(&item.location, DeleteOptions { recursive: item.kind == FileKind::Directory })
			.await?;
		progress.item_completed(item.size).await?;
	}
	Ok(())
}

async fn execute_trash<V: FileSystem>(
	vfs: &V,
	_task: &TrashTask,
	items: Vec<PreparedItem>,
	cancellation: &CancellationToken,
	progress: &mut ProgressReporter<'_>,
) -> Result<(), FsError> {
	for item in items {
		check_cancelled(cancellation, Some(item.location.clone()))?;
		progress.item_started(item.location.clone()).await?;
		vfs.trash(&item.location).await?;
		progress.item_completed(item.size).await?;
	}
	Ok(())
}

struct CopyOutcome {
	copied: bool,
}

async fn copy_file<V: FileSystem>(
	vfs: &V,
	source: &Location,
	requested: &Location,
	policy: ConflictPolicy,
	attrs: &FileAttrs,
	cancellation: &CancellationToken,
	progress: &mut ProgressReporter<'_>,
) -> Result<CopyOutcome, FsError> {
	let Some(destination) = resolve_destination(vfs, requested, policy).await? else {
		return Ok(CopyOutcome { copied: false });
	};
	if destination == *source {
		return Err(FsError::new(
			FsErrorCode::InvalidInput,
			"source and destination are the same entry",
			Some(destination),
		));
	}
	check_cancelled(cancellation, Some(source.clone()))?;
	let temporary = vfs.create_temp(&destination).await?;
	let result = copy_into_temp(vfs, source, &temporary, attrs, cancellation, progress).await;
	if let Err(error) = result {
		return Err(clean_after_error(vfs, &temporary, error).await);
	}
	if let Err(error) = check_cancelled(cancellation, Some(source.clone())) {
		return Err(clean_after_error(vfs, &temporary, error).await);
	}
	if let Err(error) = vfs.rename(&temporary, &destination).await {
		return Err(clean_after_error(vfs, &temporary, error).await);
	}
	Ok(CopyOutcome { copied: true })
}

async fn copy_into_temp<V: FileSystem>(
	vfs: &V,
	source: &Location,
	temporary: &Location,
	attrs: &FileAttrs,
	cancellation: &CancellationToken,
	progress: &mut ProgressReporter<'_>,
) -> Result<(), FsError> {
	let mut reader = vfs.open_read(source).await?;
	let mut writer = vfs
		.open_write(temporary, OpenWriteOptions {
			create:     false,
			create_new: false,
			truncate:   true,
			append:     false,
			attributes: None,
		})
		.await?;
	let mut buffer = vec![0; COPY_BUFFER_SIZE];
	loop {
		let read = tokio::select! {
			biased;
			() = cancellation.cancelled() => return Err(cancelled_error(Some(source.clone()))),
			result = reader.read(&mut buffer) => result.map_err(|error| FsError::from_io(error, Some(source.clone())))?,
		};
		if read == 0 {
			break;
		}
		tokio::select! {
			biased;
			() = cancellation.cancelled() => return Err(cancelled_error(Some(source.clone()))),
			result = writer.write_all(&buffer[..read]) => result.map_err(|error| FsError::from_io(error, Some(temporary.clone())))?,
		}
		progress.advance(read as u64).await?;
		#[cfg(test)]
		tokio::time::sleep(Duration::from_millis(1)).await;
	}
	writer.flush().await.map_err(|error| FsError::from_io(error, Some(temporary.clone())))?;
	writer.shutdown().await.map_err(|error| FsError::from_io(error, Some(temporary.clone())))?;
	drop(writer);
	progress.flush().await?;
	vfs.set_attrs(temporary, attrs).await
}

async fn clean_after_error<V: FileSystem>(
	vfs: &V,
	temporary: &Location,
	original: FsError,
) -> FsError {
	match vfs.delete(temporary, DeleteOptions { recursive: false }).await {
		Ok(()) => original,
		Err(cleanup) if cleanup.code == FsErrorCode::NotFound => original,
		Err(cleanup) => FsError::new(
			cleanup.code,
			format!("{}; staging cleanup also failed: {}", original.message, cleanup.message),
			Some(temporary.clone()),
		),
	}
}

async fn resolve_destination<V: FileSystem>(
	vfs: &V,
	destination: &Location,
	policy: ConflictPolicy,
) -> Result<Option<Location>, FsError> {
	match vfs.stat(destination, StatOptions { follow_symlinks: false }).await {
		Err(error) if error.code == FsErrorCode::NotFound => return Ok(Some(destination.clone())),
		Err(error) => return Err(error),
		Ok(_) => {}
	}
	match policy {
		ConflictPolicy::Overwrite => Ok(Some(destination.clone())),
		ConflictPolicy::Skip => Ok(None),
		ConflictPolicy::Ask | ConflictPolicy::Abort => Err(FsError::new(
			FsErrorCode::AlreadyExists,
			"destination already exists",
			Some(destination.clone()),
		)),
		ConflictPolicy::Rename => {
			let parent = destination.parent().ok_or_else(|| {
				FsError::new(
					FsErrorCode::InvalidInput,
					"destination has no parent",
					Some(destination.clone()),
				)
			})?;
			let name = file_name(destination)?;
			for suffix in 1..=10_000_u32 {
				let mut bytes = name.as_bytes().to_vec();
				bytes.extend_from_slice(format!(".{suffix}").as_bytes());
				let candidate = parent.join(NativePath::new(bytes));
				match vfs.stat(&candidate, StatOptions { follow_symlinks: false }).await {
					Err(error) if error.code == FsErrorCode::NotFound => return Ok(Some(candidate)),
					Err(error) => return Err(error),
					Ok(_) => {}
				}
			}
			Err(FsError::new(
				FsErrorCode::AlreadyExists,
				"could not find an unused destination name",
				Some(destination.clone()),
			))
		}
	}
}

fn file_name(location: &Location) -> Result<NativePath, FsError> {
	location.path.file_name().ok_or_else(|| {
		FsError::new(FsErrorCode::InvalidInput, "location has no file name", Some(location.clone()))
	})
}

fn check_cancelled(token: &CancellationToken, location: Option<Location>) -> Result<(), FsError> {
	if token.is_cancelled() { Err(cancelled_error(location)) } else { Ok(()) }
}

fn cancelled_error(location: Option<Location>) -> FsError {
	FsError::new(FsErrorCode::Cancelled, "task was cancelled", location)
}

async fn send_update(
	updates: &mpsc::Sender<WorkerUpdate>,
	update: WorkerUpdate,
) -> Result<(), FsError> {
	updates.send(update).await.map_err(|_| {
		FsError::new(FsErrorCode::Cancelled, "scheduler stopped while task was running", None)
	})
}

struct ProgressReporter<'a> {
	id:      TaskId,
	updates: &'a mpsc::Sender<WorkerUpdate>,
	pending: u64,
	last:    Instant,
}

impl<'a> ProgressReporter<'a> {
	fn new(id: TaskId, updates: &'a mpsc::Sender<WorkerUpdate>) -> Self {
		Self { id, updates, pending: 0, last: Instant::now() }
	}

	async fn item_started(&mut self, location: Location) -> Result<(), FsError> {
		send_update(self.updates, WorkerUpdate::ItemStarted { id: self.id, location }).await
	}

	async fn advance(&mut self, bytes: u64) -> Result<(), FsError> {
		self.pending = self.pending.saturating_add(bytes);
		if self.last.elapsed() >= PROGRESS_INTERVAL {
			self.flush().await?;
		}
		Ok(())
	}

	async fn flush(&mut self) -> Result<(), FsError> {
		if self.pending > 0 {
			let bytes = std::mem::take(&mut self.pending);
			send_update(self.updates, WorkerUpdate::Bytes { id: self.id, bytes }).await?;
		}
		self.last = Instant::now();
		Ok(())
	}

	async fn item_completed(&mut self, accounted_bytes: u64) -> Result<(), FsError> {
		self.flush().await?;
		send_update(self.updates, WorkerUpdate::ItemCompleted { id: self.id, accounted_bytes }).await
	}
}

#[cfg(test)]
mod tests {
	use std::{fs, path::{Path, PathBuf}, sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}};

	use thallium_fs_core::{ConflictPolicy, FsErrorCode, Location, NativePath};
	use thallium_protocol::TaskStatus;
	use thallium_vfs::LocalBackend;

	use super::{Scheduler, SchedulerConfig};
	use crate::{CopyTask, Priority, TaskInput, TaskOptions};

	struct TestDirectory(PathBuf);

	impl TestDirectory {
		fn new(label: &str) -> Self {
			let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_nanos();
			let path = std::env::temp_dir()
				.join(format!("thallium-scheduler-{label}-{}-{nonce}", std::process::id()));
			fs::create_dir_all(&path).expect("create test directory");
			Self(path)
		}

		fn location(&self) -> Location { Location::local(NativePath::from(self.0.as_path())) }
	}

	impl Drop for TestDirectory {
		fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
	}

	fn config() -> SchedulerConfig {
		SchedulerConfig {
			active_mutation_tasks: 1,
			command_capacity: 16,
			worker_event_capacity: 16,
			retained_terminal_tasks: 16,
			..SchedulerConfig::default()
		}
	}

	fn copy_input(source: &Path, destination: &TestDirectory) -> TaskInput {
		TaskInput::Copy(CopyTask {
			sources:     vec![Location::local(NativePath::from(source))],
			destination: destination.location(),
			options:     TaskOptions {
				priority:        Priority::Normal,
				conflict_policy: ConflictPolicy::Overwrite,
				attributes:      None,
			},
		})
	}

	#[tokio::test]
	async fn successful_copy_reaches_one_hundred_percent() {
		let source_dir = TestDirectory::new("success-source");
		let destination = TestDirectory::new("success-destination");
		let source = source_dir.0.join("payload.bin");
		fs::write(&source, vec![7; 256 * 1024]).expect("write source");
		let scheduler = Scheduler::new(Arc::new(LocalBackend::new()), config());
		let mut handle = scheduler.add(copy_input(&source, &destination)).await.expect("submit copy");

		let terminal =
			tokio::time::timeout(Duration::from_secs(5), handle.wait()).await.expect("task timeout");
		assert_eq!(terminal.status, TaskStatus::Succeeded);
		assert_eq!(terminal.progress.basis_points(), 10_000);
		assert_eq!(fs::read(destination.0.join("payload.bin")).expect("read copy"), vec![
			7;
			256 * 1024
		]);
		scheduler.shutdown().await.expect("shutdown");
	}

	#[tokio::test]
	async fn task_failure_is_readable_from_the_handle() {
		let source_dir = TestDirectory::new("failure-source");
		let destination = TestDirectory::new("failure-destination");
		let source = source_dir.0.join("missing.bin");
		let scheduler = Scheduler::new(Arc::new(LocalBackend::new()), config());
		let mut handle = scheduler.add(copy_input(&source, &destination)).await.expect("submit copy");

		let terminal =
			tokio::time::timeout(Duration::from_secs(5), handle.wait()).await.expect("task timeout");
		assert_eq!(terminal.status, TaskStatus::Failed);
		assert_eq!(terminal.issue.expect("failure detail").code, FsErrorCode::NotFound);
		scheduler.shutdown().await.expect("shutdown");
	}

	#[tokio::test]
	async fn saturated_event_queue_forces_authoritative_resync() {
		let source_dir = TestDirectory::new("resync-source");
		let destination = TestDirectory::new("resync-destination");
		let source = source_dir.0.join("missing.bin");
		let scheduler = Scheduler::new(Arc::new(LocalBackend::new()), SchedulerConfig {
			worker_event_capacity: 1,
			..config()
		});
		let mut handle = scheduler.add(copy_input(&source, &destination)).await.expect("submit copy");
		let terminal =
			tokio::time::timeout(Duration::from_secs(5), handle.wait()).await.expect("task timeout");
		assert_eq!(terminal.status, TaskStatus::Failed);

		let event = tokio::time::timeout(Duration::from_secs(5), scheduler.next_event())
			.await
			.expect("event timeout")
			.expect("event stream");
		assert_eq!(event, crate::SchedulerEvent::ResyncRequired);
		let snapshots = scheduler.snapshots().await.expect("resync snapshots");
		assert_eq!(snapshots, [terminal]);
		scheduler.shutdown().await.expect("shutdown");
	}

	#[tokio::test]
	async fn cancelling_copy_removes_staging_and_never_publishes_final_file() {
		let source_dir = TestDirectory::new("cancel-source");
		let destination = TestDirectory::new("cancel-destination");
		let source = source_dir.0.join("large.bin");
		fs::write(&source, vec![9; 8 * 1024 * 1024]).expect("write source");
		let scheduler = Scheduler::new(Arc::new(LocalBackend::new()), config());
		let mut handle = scheduler.add(copy_input(&source, &destination)).await.expect("submit copy");

		tokio::time::timeout(Duration::from_secs(5), async {
			loop {
				let snapshot = handle.snapshot();
				if snapshot.progress.bytes_completed > 0 {
					break;
				}
				handle.changed().await.expect("scheduler still running");
			}
		})
		.await
		.expect("copy did not report progress");
		handle.cancel();
		let terminal =
			tokio::time::timeout(Duration::from_secs(5), handle.wait()).await.expect("cancel timeout");
		assert_eq!(terminal.status, TaskStatus::Cancelled);
		assert_eq!(terminal.issue.expect("cancel detail").code, FsErrorCode::Cancelled);
		assert!(!destination.0.join("large.bin").exists());
		let leftovers: Vec<_> = fs::read_dir(&destination.0)
			.expect("scan destination")
			.filter_map(Result::ok)
			.filter(|entry| entry.file_name().to_string_lossy().starts_with(".thallium-"))
			.collect();
		assert!(leftovers.is_empty(), "staging files remain: {leftovers:?}");
		scheduler.shutdown().await.expect("shutdown");
	}
}
