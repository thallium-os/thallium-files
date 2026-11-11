//! The sole asynchronous coordinator and owner of [`BrowserState`].

use std::{fmt, sync::Arc};

use thallium_fs_core::{BackendId, FsError, FsErrorCode, Location, NativePath};
use thallium_protocol::{BrowserStateSnapshot, StateDelta, TaskInfo};
use thallium_scheduler::{CopyTask, CreateTask, DeleteTask, LinkTask, MoveTask, Priority, RenameTask, Scheduler, SchedulerError, SchedulerEvent, TaskInput, TaskOptions, TrashTask};
use thallium_state::{BrowserState, Reduction, StateAction, StateEffect, TaskRequest, reduce};
use thallium_vfs::{FileSystem, LocalBackend};
use thallium_watcher::{WatchedSet, Watcher};
use tokio::{sync::{broadcast, mpsc, oneshot}, task::JoinHandle};

use crate::{Action, Command, CommandEnvelope, EffectError, EffectFeedback, EffectKind, EffectResult, ValidatedAction, ValidationError, ValidationResult};

/// Bounds for the coordinator-owned service and frontend queues.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoordinatorConfig {
	pub feedback_capacity:    usize,
	pub state_delta_capacity: usize,
	pub watcher_capacity:     usize,
}

impl Default for CoordinatorConfig {
	fn default() -> Self {
		Self { feedback_capacity: 256, state_delta_capacity: 256, watcher_capacity: 16 }
	}
}

/// Error returned by command dispatch or service pumping.
#[derive(Debug)]
pub enum ActionError {
	Validation(ValidationError),
	Effect(EffectError),
	FeedbackClosed,
	ServiceStopped(&'static str),
}

impl fmt::Display for ActionError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Validation(error) => error.fmt(formatter),
			Self::Effect(error) => error.fmt(formatter),
			Self::FeedbackClosed => formatter.write_str("the action feedback loop is closed"),
			Self::ServiceStopped(service) => write!(formatter, "the {service} service stopped"),
		}
	}
}

impl std::error::Error for ActionError {}

impl From<ValidationError> for ActionError {
	fn from(error: ValidationError) -> Self { Self::Validation(error) }
}

impl From<EffectError> for ActionError {
	fn from(error: EffectError) -> Self { Self::Effect(error) }
}

/// State changes produced while dispatching one command or accepting feedback.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DispatchReport {
	pub deltas:         Vec<StateDelta>,
	pub submitted_task: Option<TaskInfo>,
}

/// Result of accepting one background service observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedbackReport {
	pub feedback: EffectFeedback,
	pub deltas:   Vec<StateDelta>,
}

enum WatcherCommand {
	Synchronize(
		WatchedSet,
		thallium_fs_core::ListingGeneration,
		oneshot::Sender<Result<thallium_watcher::WatchSetDiff, FsError>>,
	),
	Refresh(Location, oneshot::Sender<Result<(), EffectError>>),
	Shutdown(oneshot::Sender<Result<(), FsError>>),
}

/// Composition of state ownership and renderer-neutral service handles.
pub struct Coordinator<V: FileSystem = LocalBackend> {
	state:          BrowserState,
	vfs:            Arc<V>,
	scheduler:      Arc<Scheduler<V>>,
	watcher_tx:     mpsc::Sender<WatcherCommand>,
	feedback_rx:    mpsc::Receiver<EffectFeedback>,
	state_deltas:   broadcast::Sender<StateDelta>,
	watcher_task:   Option<JoinHandle<()>>,
	scheduler_task: Option<JoinHandle<()>>,
}

impl<V: FileSystem + 'static> Coordinator<V> {
	/// Construct the single state owner and start its service observation pumps.
	/// This must be called from the backend-owned Tokio runtime.
	#[must_use]
	pub fn new(
		state: BrowserState,
		vfs: Arc<V>,
		scheduler: Scheduler<V>,
		watcher: Watcher<V>,
		config: CoordinatorConfig,
	) -> Self {
		let (feedback_tx, feedback_rx) = mpsc::channel(config.feedback_capacity.max(1));
		let (watcher_tx, watcher_rx) = mpsc::channel(config.watcher_capacity.max(1));
		let (state_deltas, _) = broadcast::channel(config.state_delta_capacity.max(1));
		let scheduler = Arc::new(scheduler);
		let watcher_task = tokio::spawn(run_watcher(watcher, watcher_rx, feedback_tx.clone()));
		let scheduler_task = tokio::spawn(run_scheduler(Arc::clone(&scheduler), feedback_tx.clone()));
		Self {
			state,
			vfs,
			scheduler,
			watcher_tx,
			feedback_rx,
			state_deltas,
			watcher_task: Some(watcher_task),
			scheduler_task: Some(scheduler_task),
		}
	}

	/// Observe browser state without exposing mutable access.
	#[must_use]
	pub const fn state(&self) -> &BrowserState { &self.state }

	/// Return a complete bridge-safe recovery snapshot.
	#[must_use]
	pub fn snapshot(&self) -> BrowserStateSnapshot { self.state.snapshot() }

	/// Subscribe to bounded, contiguous frontend state updates.
	///
	/// A lagged subscriber must recover by calling [`Self::snapshot`].
	#[must_use]
	pub fn subscribe_state(&self) -> broadcast::Receiver<StateDelta> { self.state_deltas.subscribe() }

	/// Dispatch a command against the current state revision.
	pub async fn dispatch_command(
		&mut self,
		command: Command,
	) -> Result<DispatchReport, ActionError> {
		self.dispatch(CommandEnvelope { expected_revision: self.state.revision, command }).await
	}

	/// Validate, reduce, invoke owned effects, and feed their results back.
	pub async fn dispatch(
		&mut self,
		command: CommandEnvelope,
	) -> Result<DispatchReport, ActionError> {
		let validated = self.validate(&command)?;
		let mut report = DispatchReport::default();
		match validated.action {
			Action::Reduce(action) => {
				let effects = self.install_reduction(reduce(&self.state, *action), &mut report);
				self.invoke_effects(effects, &mut report).await?;
			}
			Action::Invoke(effect) => self.invoke_effects(vec![effect], &mut report).await?,
			Action::CancelTask(id) => {
				self.scheduler.cancel(id).await.map_err(scheduler_error)?;
			}
			Action::DismissTask(id) => {
				self.scheduler.dismiss(id).await.map_err(scheduler_error)?;
				self.accept_feedback(EffectFeedback::TaskDismissed(id), &mut report);
			}
		}
		Ok(report)
	}

	/// Wait for and reduce one watcher or scheduler observation.
	pub async fn pump_feedback(&mut self) -> Result<FeedbackReport, ActionError> {
		let feedback = self.feedback_rx.recv().await.ok_or(ActionError::FeedbackClosed)?;
		self.process_background_feedback(feedback)
	}

	/// Reduce one already-buffered service observation without waiting.
	pub fn try_pump_feedback(&mut self) -> Result<Option<FeedbackReport>, ActionError> {
		match self.feedback_rx.try_recv() {
			Ok(feedback) => self.process_background_feedback(feedback).map(Some),
			Err(mpsc::error::TryRecvError::Empty) => Ok(None),
			Err(mpsc::error::TryRecvError::Disconnected) => Err(ActionError::FeedbackClosed),
		}
	}

	/// Validate a command using state only; no service call or mutation occurs.
	pub fn validate(&self, envelope: &CommandEnvelope) -> ValidationResult {
		if envelope.expected_revision != self.state.revision {
			return Err(ValidationError::StaleRevision {
				expected: envelope.expected_revision,
				actual:   self.state.revision,
			});
		}

		let action = match &envelope.command {
			Command::Navigate(location) => {
				validate_locations(std::slice::from_ref(location))?;
				Action::Reduce(Box::new(StateAction::NavigateTo(location.clone())))
			}
			Command::NavigateBack if self.state.history.can_go_back() => {
				Action::Reduce(Box::new(StateAction::NavigateBack))
			}
			Command::NavigateForward if self.state.history.can_go_forward() => {
				Action::Reduce(Box::new(StateAction::NavigateForward))
			}
			Command::NavigateBack | Command::NavigateForward => return Err(ValidationError::NoHistory),
			Command::Refresh => {
				let location =
					self.state.current_location.clone().ok_or(ValidationError::NoCurrentLocation)?;
				Action::Invoke(StateEffect::RefreshDirectory { location })
			}
			Command::SetSelection { selected, focused } => {
				for id in selected.iter().chain(focused) {
					if !self.state.directory.entries.contains_key(id) {
						return Err(ValidationError::UnknownEntry(*id));
					}
				}
				Action::Reduce(Box::new(StateAction::SetSelection {
					selected: selected.clone(),
					focused:  *focused,
				}))
			}
			Command::ToggleSelection(id) => {
				if !self.state.directory.entries.contains_key(id) {
					return Err(ValidationError::UnknownEntry(*id));
				}
				Action::Reduce(Box::new(StateAction::ToggleSelection(*id)))
			}
			Command::ClearSelection => Action::Reduce(Box::new(StateAction::ClearSelection)),
			Command::Copy { sources, destination, conflict_policy } => {
				validate_task_locations(sources, Some(destination))?;
				Action::Invoke(StateEffect::SubmitTask(TaskRequest {
					kind:            thallium_protocol::TaskKind::Copy,
					sources:         sources.clone(),
					destination:     Some(destination.clone()),
					conflict_policy: *conflict_policy,
				}))
			}
			Command::Move { sources, destination, conflict_policy } => {
				validate_task_locations(sources, Some(destination))?;
				Action::Invoke(StateEffect::SubmitTask(TaskRequest {
					kind:            thallium_protocol::TaskKind::Move,
					sources:         sources.clone(),
					destination:     Some(destination.clone()),
					conflict_policy: *conflict_policy,
				}))
			}
			Command::Delete { targets } => {
				validate_task_locations(targets, None)?;
				Action::Invoke(StateEffect::SubmitTask(TaskRequest {
					kind:            thallium_protocol::TaskKind::Delete,
					sources:         targets.clone(),
					destination:     None,
					conflict_policy: thallium_fs_core::ConflictPolicy::Abort,
				}))
			}
			Command::Trash { targets } => {
				validate_task_locations(targets, None)?;
				Action::Invoke(StateEffect::SubmitTask(TaskRequest {
					kind:            thallium_protocol::TaskKind::Trash,
					sources:         targets.clone(),
					destination:     None,
					conflict_policy: thallium_fs_core::ConflictPolicy::Abort,
				}))
			}
			Command::Rename { source, name } => {
				validate_locations(std::slice::from_ref(source))?;
				validate_name(name)?;
				let parent = source.parent().ok_or(ValidationError::InvalidLocation)?;
				Action::Invoke(StateEffect::SubmitTask(TaskRequest {
					kind:            thallium_protocol::TaskKind::Rename,
					sources:         vec![source.clone()],
					destination:     Some(parent.join(name)),
					conflict_policy: thallium_fs_core::ConflictPolicy::Abort,
				}))
			}
			Command::CreateFolder { parent, name } => {
				validate_locations(std::slice::from_ref(parent))?;
				validate_name(name)?;
				Action::Invoke(StateEffect::SubmitTask(TaskRequest {
					kind:            thallium_protocol::TaskKind::CreateFolder,
					sources:         vec![parent.join(name)],
					destination:     None,
					conflict_policy: thallium_fs_core::ConflictPolicy::Abort,
				}))
			}
			Command::CreateFile { parent, name, template } => {
				validate_locations(std::slice::from_ref(parent))?;
				validate_locations(template.as_slice())?;
				validate_name(name)?;
				Action::Invoke(StateEffect::SubmitTask(TaskRequest {
					kind:            thallium_protocol::TaskKind::CreateFile,
					sources:         vec![parent.join(name)],
					destination:     template.clone(),
					conflict_policy: thallium_fs_core::ConflictPolicy::Abort,
				}))
			}
			Command::Link { sources, destination } => {
				validate_task_locations(sources, Some(destination))?;
				Action::Invoke(StateEffect::SubmitTask(TaskRequest {
					kind:            thallium_protocol::TaskKind::Link,
					sources:         sources.clone(),
					destination:     Some(destination.clone()),
					conflict_policy: thallium_fs_core::ConflictPolicy::Rename,
				}))
			}
			Command::CancelTask(id) => {
				let task = self.state.tasks.get(id).ok_or(ValidationError::UnknownTask(*id))?;
				if !task.can_cancel() {
					return Err(ValidationError::TaskNotCancellable(*id));
				}
				Action::CancelTask(*id)
			}
			Command::DismissTask(id) => {
				let task = self.state.tasks.get(id).ok_or(ValidationError::UnknownTask(*id))?;
				if !task.status.is_terminal() {
					return Err(ValidationError::TaskNotTerminal(*id));
				}
				Action::DismissTask(*id)
			}
		};
		Ok(ValidatedAction { validated_at: self.state.revision, action })
	}

	/// Stop admission, release the native watch, and join both observation pumps.
	pub async fn shutdown(&mut self) -> Result<(), ActionError> {
		if let Some(watcher_task) = self.watcher_task.take() {
			let (reply, response) = oneshot::channel();
			self
				.watcher_tx
				.send(WatcherCommand::Shutdown(reply))
				.await
				.map_err(|_| ActionError::ServiceStopped("watcher"))?;
			tokio::pin!(response);
			loop {
				tokio::select! {
					result = &mut response => {
						result.map_err(|_| ActionError::ServiceStopped("watcher"))?
							.map_err(|error| effect_from_fs(EffectKind::Watcher, error))?;
						break;
					}
					feedback = self.feedback_rx.recv() => {
						if let Some(feedback) = feedback {
							let _ = self.process_background_feedback(feedback)?;
						}
					}
				}
			}
			watcher_task.await.map_err(|_| ActionError::ServiceStopped("watcher"))?;
		}

		self.scheduler.shutdown().await.map_err(scheduler_error)?;
		if let Some(mut scheduler_task) = self.scheduler_task.take() {
			loop {
				tokio::select! {
					result = &mut scheduler_task => {
						result.map_err(|_| ActionError::ServiceStopped("scheduler"))?;
						break;
					}
					feedback = self.feedback_rx.recv() => {
						if let Some(feedback) = feedback {
							let _ = self.process_background_feedback(feedback)?;
						}
					}
				}
			}
		}
		Ok(())
	}

	async fn invoke_effects(
		&mut self,
		mut effects: Vec<StateEffect>,
		report: &mut DispatchReport,
	) -> Result<(), ActionError> {
		// Install the watch before the navigation scan. That closes the only gap
		// in which a mutation could occur between the visible scan and inotify
		// registration; the watch's baseline and state scan then agree.
		effects.sort_by_key(|effect| !matches!(effect, StateEffect::WatchLocations { .. }));
		for effect in effects {
			let feedback = self.invoke_effect(effect).await?;
			if let EffectFeedback::TaskSubmitted(task) = &feedback {
				report.submitted_task = Some(task.clone());
			}
			self.accept_feedback(feedback, report);
		}
		Ok(())
	}

	async fn invoke_effect(&self, effect: StateEffect) -> EffectResult {
		match effect {
			StateEffect::ScanDirectory { location, generation } => match self.vfs.scan(&location).await {
				Ok(entries) => Ok(EffectFeedback::DirectoryScanned { location, generation, entries }),
				Err(error) => Ok(EffectFeedback::DirectoryScanFailed { location, generation, error }),
			},
			StateEffect::WatchLocations { locations, generation } => {
				// The watcher is inotify over the local filesystem. A remote has
				// no equivalent to offer it, so those locations go unwatched and
				// are refreshed when they are navigated to instead.
				let desired = WatchedSet::new(
					locations
						.into_iter()
						.filter(|location| location.backend == BackendId::LOCAL)
						.map(thallium_watcher::Watchee::local),
				);
				let (reply, response) = oneshot::channel();
				self.send_watcher(WatcherCommand::Synchronize(desired, generation, reply))?;
				let diff = response
					.await
					.map_err(|_| service_stopped(EffectKind::Watcher))?
					.map_err(|error| effect_from_fs(EffectKind::Watcher, error))?;
				Ok(EffectFeedback::WatchSetSynchronized(diff))
			}
			StateEffect::RefreshDirectory { location } => {
				let (reply, response) = oneshot::channel();
				self.send_watcher(WatcherCommand::Refresh(location.clone(), reply))?;
				response.await.map_err(|_| service_stopped(EffectKind::Watcher))??;
				Ok(EffectFeedback::WatcherReconciled { deltas: Vec::new() })
			}
			StateEffect::SubmitTask(request) => {
				let handle = self.scheduler.add(task_input(request)?).await.map_err(scheduler_error)?;
				Ok(EffectFeedback::TaskSubmitted(handle.snapshot()))
			}
			StateEffect::RequestConflictDecision { .. } => Err(EffectError {
				kind:     EffectKind::Scheduler,
				code:     Some(FsErrorCode::Unsupported),
				message:  "interactive conflict decisions are not implemented in the MVP".into(),
				location: None,
			}),
		}
	}

	fn send_watcher(&self, command: WatcherCommand) -> Result<(), EffectError> {
		self.watcher_tx.try_send(command).map_err(|error| EffectError {
			kind:     EffectKind::Watcher,
			code:     Some(match error {
				mpsc::error::TrySendError::Full(_) => FsErrorCode::Busy,
				mpsc::error::TrySendError::Closed(_) => FsErrorCode::Cancelled,
			}),
			message:  "watcher command queue is unavailable".into(),
			location: None,
		})
	}

	fn process_background_feedback(
		&mut self,
		feedback: EffectFeedback,
	) -> Result<FeedbackReport, ActionError> {
		if let EffectFeedback::ServiceFailed(error) = feedback {
			return Err(ActionError::Effect(error));
		}
		let mut report = DispatchReport::default();
		self.accept_feedback(feedback.clone(), &mut report);
		Ok(FeedbackReport { feedback, deltas: report.deltas })
	}

	fn accept_feedback(&mut self, feedback: EffectFeedback, report: &mut DispatchReport) {
		let actions = match feedback {
			EffectFeedback::DirectoryScanned { location, generation, entries } => {
				vec![StateAction::DirectoryScanned {
					location,
					generation,
					entries,
					complete: true,
					error: None,
				}]
			}
			EffectFeedback::DirectoryScanFailed { location, generation, error } => {
				vec![StateAction::DirectoryScanned {
					location,
					generation,
					entries: Vec::new(),
					complete: false,
					error: Some(error),
				}]
			}
			EffectFeedback::WatcherReconciled { deltas } => {
				deltas.into_iter().map(StateAction::ApplyFsDelta).collect()
			}
			EffectFeedback::TaskSubmitted(task) => vec![StateAction::UpdateTask(task)],
			EffectFeedback::SchedulerEvent(SchedulerEvent::Snapshot(task)) => {
				vec![StateAction::UpdateTask(*task)]
			}
			EffectFeedback::SchedulerEvent(SchedulerEvent::Dismissed(id))
			| EffectFeedback::TaskDismissed(id) => vec![StateAction::RemoveTask(id)],
			EffectFeedback::SchedulerResynchronized(tasks) => {
				let retained: std::collections::BTreeSet<_> = tasks.iter().map(|task| task.id).collect();
				let mut actions: Vec<_> = self
					.state
					.task_order
					.iter()
					.filter(|id| !retained.contains(id))
					.copied()
					.map(StateAction::RemoveTask)
					.collect();
				actions.extend(tasks.into_iter().map(StateAction::UpdateTask));
				actions
			}
			EffectFeedback::SchedulerEvent(SchedulerEvent::ResyncRequired)
			| EffectFeedback::WatchSetSynchronized(_)
			| EffectFeedback::TaskCancelled(_)
			| EffectFeedback::ServiceFailed(_) => Vec::new(),
		};

		for action in actions {
			let effects = self.install_reduction(reduce(&self.state, action), report);
			debug_assert!(effects.is_empty(), "service feedback must not recursively start I/O");
		}
	}

	fn install_reduction(
		&mut self,
		reduction: Reduction,
		report: &mut DispatchReport,
	) -> Vec<StateEffect> {
		self.state = reduction.state;
		if let Some(delta) = reduction.delta {
			let _ = self.state_deltas.send(delta.clone());
			report.deltas.push(delta);
		}
		reduction.effects
	}
}

async fn run_scheduler<V: FileSystem + 'static>(
	scheduler: Arc<Scheduler<V>>,
	feedback: mpsc::Sender<EffectFeedback>,
) {
	loop {
		match scheduler.next_event().await {
			Ok(SchedulerEvent::ResyncRequired) => match scheduler.snapshots().await {
				Ok(snapshots) => {
					if feedback.send(EffectFeedback::SchedulerResynchronized(snapshots)).await.is_err() {
						break;
					}
				}
				Err(error) => {
					let _ = feedback.send(EffectFeedback::ServiceFailed(scheduler_error(error))).await;
					break;
				}
			},
			Ok(event) => {
				if feedback.send(EffectFeedback::SchedulerEvent(event)).await.is_err() {
					break;
				}
			}
			Err(SchedulerError::Unavailable) => break,
			Err(error) => {
				let _ = feedback.send(EffectFeedback::ServiceFailed(scheduler_error(error))).await;
				break;
			}
		}
	}
}

async fn run_watcher<V: FileSystem>(
	mut watcher: Watcher<V>,
	mut commands: mpsc::Receiver<WatcherCommand>,
	feedback: mpsc::Sender<EffectFeedback>,
) {
	loop {
		tokio::select! {
			biased;
			command = commands.recv() => match command {
				Some(WatcherCommand::Synchronize(desired, generation, reply)) => {
					let _ = reply.send(watcher.sync_watch_set(desired, generation).await);
				}
				Some(WatcherCommand::Refresh(location, reply)) => {
					let result = watcher.request_refresh(location.clone()).map_err(|error| EffectError {
						kind: EffectKind::Watcher,
						code: Some(match error {
							thallium_watcher::ReportError::Full => FsErrorCode::Busy,
							thallium_watcher::ReportError::Closed => FsErrorCode::Cancelled,
						}),
						message: "watcher refresh queue is unavailable".into(),
						location: Some(location),
					});
					let _ = reply.send(result);
				}
				Some(WatcherCommand::Shutdown(reply)) => {
					let result = watcher.shutdown();
					let _ = reply.send(result);
					break;
				}
				None => {
					let _ = watcher.shutdown();
					break;
				}
			},
			result = watcher.next_deltas() => match result {
				Ok(Some(result)) => {
					if feedback.send(EffectFeedback::WatcherReconciled { deltas: result.deltas }).await.is_err() {
						break;
					}
				}
				Ok(None) => break,
				Err(error) => {
					if feedback.send(EffectFeedback::ServiceFailed(effect_from_fs(EffectKind::Watcher, error))).await.is_err() {
						break;
					}
				}
			}
		}
	}
}

/// A location is addressable if its path is absolute. Which backend answers for
/// it is the router's business, not this one's: rejecting anything but
/// [`BackendId::LOCAL`] here made every remote unreachable, since a command
/// naming one never survived validation to be dispatched at all.
fn validate_locations(locations: &[Location]) -> Result<(), ValidationError> {
	if locations.iter().any(|location| !location.is_absolute()) {
		return Err(ValidationError::InvalidLocation);
	}
	Ok(())
}

fn validate_name(name: &NativePath) -> Result<(), ValidationError> {
	let bytes = name.as_bytes();
	if bytes.is_empty()
		|| bytes == b"."
		|| bytes == b".."
		|| bytes.contains(&b'/')
		|| bytes.contains(&0)
	{
		return Err(ValidationError::InvalidName);
	}
	Ok(())
}

fn validate_task_locations(
	targets: &[Location],
	destination: Option<&Location>,
) -> Result<(), ValidationError> {
	if targets.is_empty() {
		return Err(ValidationError::EmptyTargets);
	}
	validate_locations(targets)?;
	if let Some(destination) = destination {
		validate_locations(std::slice::from_ref(destination))?;
	}
	Ok(())
}

fn task_input(request: TaskRequest) -> Result<TaskInput, EffectError> {
	let options = TaskOptions {
		priority:        Priority::Normal,
		conflict_policy: request.conflict_policy,
		attributes:      None,
	};
	match request.kind {
		thallium_protocol::TaskKind::Copy => Ok(TaskInput::Copy(CopyTask {
			sources: request.sources,
			destination: request.destination.ok_or_else(missing_destination)?,
			options,
		})),
		thallium_protocol::TaskKind::Move => Ok(TaskInput::Move(MoveTask {
			sources: request.sources,
			destination: request.destination.ok_or_else(missing_destination)?,
			options,
		})),
		thallium_protocol::TaskKind::Delete => {
			Ok(TaskInput::Delete(DeleteTask { targets: request.sources, priority: Priority::Normal }))
		}
		thallium_protocol::TaskKind::Trash => {
			Ok(TaskInput::Trash(TrashTask { targets: request.sources, priority: Priority::Normal }))
		}
		thallium_protocol::TaskKind::Rename => Ok(TaskInput::Rename(RenameTask {
			source:      single(request.sources)?,
			destination: request.destination.ok_or_else(missing_destination)?,
		})),
		thallium_protocol::TaskKind::CreateFolder => {
			Ok(TaskInput::CreateFolder(CreateTask { target: single(request.sources)?, template: None }))
		}
		thallium_protocol::TaskKind::CreateFile => Ok(TaskInput::CreateFile(CreateTask {
			target:   single(request.sources)?,
			template: request.destination,
		})),
		thallium_protocol::TaskKind::Link => Ok(TaskInput::Link(LinkTask {
			sources:     request.sources,
			destination: request.destination.ok_or_else(missing_destination)?,
		})),
	}
}

fn single(mut sources: Vec<Location>) -> Result<Location, EffectError> {
	match (sources.pop(), sources.is_empty()) {
		(Some(location), true) => Ok(location),
		_ => Err(missing_destination()),
	}
}

fn missing_destination() -> EffectError {
	EffectError {
		kind:     EffectKind::Scheduler,
		code:     Some(FsErrorCode::InvalidInput),
		message:  "copy and move tasks require a destination".into(),
		location: None,
	}
}

fn effect_from_fs(kind: EffectKind, error: FsError) -> EffectError {
	EffectError { kind, code: Some(error.code), message: error.message, location: error.location }
}

fn scheduler_error(error: SchedulerError) -> EffectError {
	EffectError {
		kind:     EffectKind::Scheduler,
		code:     None,
		message:  error.to_string(),
		location: None,
	}
}

fn service_stopped(kind: EffectKind) -> EffectError {
	EffectError {
		kind,
		code: Some(FsErrorCode::Cancelled),
		message: "service stopped before replying".into(),
		location: None,
	}
}

#[cfg(test)]
mod tests {
	use std::{path::Path, sync::Arc, time::Duration};

	use tempfile::TempDir;
	use thallium_fs_core::{ConflictPolicy, EntryId, Location, NativePath};
	use thallium_protocol::TaskStatus;
	use thallium_scheduler::{Scheduler, SchedulerConfig};
	use thallium_state::BrowserState;
	use thallium_vfs::LocalBackend;
	use thallium_watcher::{Watcher, WatcherConfig};

	use super::{Coordinator, CoordinatorConfig};
	use crate::{Command, CommandEnvelope, ValidationError};

	fn location(path: &Path) -> Location { Location::local(NativePath::from(path)) }

	fn coordinator() -> Coordinator<LocalBackend> {
		let vfs = Arc::new(LocalBackend::new());
		let scheduler = Scheduler::new(Arc::clone(&vfs), SchedulerConfig::default());
		let watcher = Watcher::new(Arc::clone(&vfs), WatcherConfig {
			notification_capacity: 64,
			max_batch_size:        64,
			batch_window:          Duration::from_millis(20),
		})
		.expect("create watcher");
		Coordinator::new(BrowserState::default(), vfs, scheduler, watcher, CoordinatorConfig::default())
	}

	#[tokio::test]
	async fn navigation_scans_and_selection_is_validated() {
		let directory = TempDir::new().expect("temporary directory");
		tokio::fs::write(directory.path().join("visible"), b"data").await.expect("create entry");
		let mut coordinator = coordinator();
		let mut deltas = coordinator.subscribe_state();

		let report = coordinator
			.dispatch_command(Command::Navigate(location(directory.path())))
			.await
			.expect("navigate");
		assert_eq!(report.deltas.len(), 2, "navigation and scan each advance state");
		assert_eq!(coordinator.state().directory.entries.len(), 1);
		let entry = coordinator.state().directory.ordered_entries().next().expect("scanned entry");
		let id = entry.id;

		coordinator.dispatch_command(Command::ToggleSelection(id)).await.expect("select entry");
		assert!(coordinator.state().selection.selected.contains(&id));
		let unknown = EntryId(u128::MAX);
		let error = coordinator
			.dispatch_command(Command::ToggleSelection(unknown))
			.await
			.expect_err("unknown entry must be rejected");
		assert!(
			matches!(error, super::ActionError::Validation(ValidationError::UnknownEntry(value)) if value == unknown)
		);

		let stale = coordinator
			.dispatch(CommandEnvelope {
				expected_revision: Default::default(),
				command:           Command::ClearSelection,
			})
			.await
			.expect_err("stale command must be rejected");
		assert!(matches!(stale, super::ActionError::Validation(ValidationError::StaleRevision { .. })));
		assert!(deltas.try_recv().is_ok(), "frontend delta is published");
		coordinator.shutdown().await.expect("shutdown");
	}

	#[tokio::test]
	async fn copy_progress_and_watcher_truth_share_the_feedback_loop() {
		let source = TempDir::new().expect("source directory");
		let destination = TempDir::new().expect("destination directory");
		let source_path = source.path().join("copied.bin");
		tokio::fs::write(&source_path, vec![7_u8; 256 * 1024]).await.expect("write source");
		let mut coordinator = coordinator();
		coordinator
			.dispatch_command(Command::Navigate(location(destination.path())))
			.await
			.expect("navigate destination");

		let submitted = coordinator
			.dispatch_command(Command::Copy {
				sources:         vec![location(&source_path)],
				destination:     location(destination.path()),
				conflict_policy: ConflictPolicy::Overwrite,
			})
			.await
			.expect("submit copy")
			.submitted_task
			.expect("initial task snapshot");
		assert!(coordinator.state().tasks.contains_key(&submitted.id));

		tokio::time::timeout(Duration::from_secs(8), async {
			loop {
				coordinator.pump_feedback().await.expect("service feedback");
				let completed = coordinator
					.state()
					.tasks
					.get(&submitted.id)
					.is_some_and(|task| task.status == TaskStatus::Succeeded);
				let visible = coordinator
					.state()
					.directory
					.ordered_entries()
					.any(|entry| entry.name.as_bytes() == b"copied.bin");
				if completed && visible {
					break;
				}
			}
		})
		.await
		.expect("copy completion and watcher delta before timeout");

		let task = &coordinator.state().tasks[&submitted.id];
		assert!(task.progress.is_complete());
		assert_eq!(
			tokio::fs::read(destination.path().join("copied.bin")).await.expect("copied file").len(),
			256 * 1024
		);
		coordinator.shutdown().await.expect("shutdown");
	}

	#[tokio::test]
	async fn watcher_create_and_delete_are_authoritative_state_changes() {
		let directory = TempDir::new().expect("temporary directory");
		let mut coordinator = coordinator();
		coordinator
			.dispatch_command(Command::Navigate(location(directory.path())))
			.await
			.expect("navigate");
		let created = directory.path().join("watched");
		tokio::fs::write(&created, b"data").await.expect("create watched file");

		tokio::time::timeout(Duration::from_secs(5), async {
			loop {
				coordinator.pump_feedback().await.expect("create feedback");
				if coordinator
					.state()
					.directory
					.ordered_entries()
					.any(|entry| entry.name.as_bytes() == b"watched")
				{
					break;
				}
			}
		})
		.await
		.expect("create delta before timeout");

		tokio::fs::remove_file(created).await.expect("delete watched file");
		tokio::time::timeout(Duration::from_secs(5), async {
			loop {
				coordinator.pump_feedback().await.expect("delete feedback");
				if !coordinator
					.state()
					.directory
					.ordered_entries()
					.any(|entry| entry.name.as_bytes() == b"watched")
				{
					break;
				}
			}
		})
		.await
		.expect("delete delta before timeout");
		coordinator.shutdown().await.expect("shutdown");
	}
}
