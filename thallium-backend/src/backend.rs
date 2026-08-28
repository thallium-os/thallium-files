//! Backend lifecycle and frontend-neutral façade.
//!
//! `PHASE2-PLAN.md` sections 1, 4, and 5 require a single action/state owner,
//! private runtime machinery, and an owned snapshot/delta surface.

use std::{sync::Arc, time::Duration};

use thallium_actions::{Command, CommandEnvelope, Coordinator, CoordinatorConfig};
use thallium_fs_core::{Location, NativePath};
use thallium_protocol::{StateDelta, StateSnapshot};
use thallium_scheduler::Scheduler;
use thallium_state::BrowserState;
use thallium_vfs::LocalBackend;
use thallium_watcher::Watcher;
use tokio::{runtime::Runtime, sync::{mpsc, oneshot}, task::JoinHandle};

use crate::{BackendConfig, BackendError, BackendResult, services::{ServiceRequest, publish}};

/// Composition root for the backend runtime and its single service graph.
pub struct Backend {
	runtime:    Option<Runtime>,
	requests:   Option<mpsc::Sender<ServiceRequest>>,
	events:     mpsc::Receiver<StateDelta>,
	owner_task: Option<JoinHandle<()>>,
}

impl Backend {
	/// Build the Tokio runtime and all services in dependency order.
	pub fn start(config: BackendConfig) -> BackendResult<Self> {
		let initial = match config.initial_location.clone() {
			Some(path) => path,
			None => NativePath::from(
				std::env::var_os("HOME")
					.ok_or_else(|| BackendError::Configuration("HOME is not set".into()))?,
			),
		};
		if !initial.is_absolute() {
			return Err(BackendError::Configuration("the initial directory must be absolute".into()));
		}

		let runtime = tokio::runtime::Builder::new_multi_thread()
			.worker_threads(config.runtime_worker_threads.max(1))
			.enable_all()
			.build()?;
		let capacity = config.event_capacity.max(1);
		let (requests, request_rx) = mpsc::channel(capacity);
		let (event_tx, events) = mpsc::channel(capacity);
		let (ready, startup) = oneshot::channel();
		let owner_task = runtime.block_on(async {
			let vfs = Arc::new(LocalBackend::new());
			let scheduler = Scheduler::new(Arc::clone(&vfs), config.scheduler);
			let watcher = Watcher::new(Arc::clone(&vfs), config.watcher)
				.map_err(|error| BackendError::Configuration(error.to_string()))?;
			let coordinator =
				Coordinator::new(BrowserState::default(), vfs, scheduler, watcher, CoordinatorConfig {
					feedback_capacity:    capacity,
					state_delta_capacity: capacity,
					watcher_capacity:     16,
				});
			Ok::<_, BackendError>(tokio::spawn(run_owner(
				coordinator,
				request_rx,
				event_tx,
				Location::local(initial.normalize()),
				ready,
			)))
		})?;
		runtime
			.block_on(startup)
			.map_err(|_| BackendError::ServiceUnavailable)?
			.map_err(BackendError::Action)?;

		Ok(Self {
			runtime: Some(runtime),
			requests: Some(requests),
			events,
			owner_task: Some(owner_task),
		})
	}

	/// Validate and enqueue a semantic command for the single action/state owner.
	pub fn submit_command(&mut self, command: CommandEnvelope) -> BackendResult<()> {
		self.request(|reply| ServiceRequest::Dispatch(command, reply))?.map_err(BackendError::Action)
	}

	/// Dispatch against the latest owner-side revision, suitable for a frontend
	/// that consumes full snapshots instead of retaining revision tokens.
	pub fn submit(&mut self, command: Command) -> BackendResult<()> {
		self
			.request(|reply| ServiceRequest::DispatchCurrent(command, reply))?
			.map_err(BackendError::Action)
	}

	/// Return an owned, revisioned recovery snapshot for the Qt adapter.
	pub fn snapshot(&self) -> BackendResult<StateSnapshot> { self.request(ServiceRequest::Snapshot) }

	/// Return the next owned state change without exposing the private event
	/// port.
	pub fn next_event(&mut self) -> BackendResult<Option<StateDelta>> {
		match self.events.try_recv() {
			Ok(delta) => Ok(Some(delta)),
			Err(mpsc::error::TryRecvError::Empty) => Ok(None),
			Err(mpsc::error::TryRecvError::Disconnected) => Err(BackendError::ServiceUnavailable),
		}
	}

	/// Stop admission, shut services down in reverse order, and join the runtime.
	pub fn shutdown(&mut self) -> BackendResult<()> {
		if self.requests.is_none() {
			return Ok(());
		}
		let result = self.request(ServiceRequest::Shutdown)?;
		self.requests.take();
		if let (Some(runtime), Some(task)) = (self.runtime.as_ref(), self.owner_task.take()) {
			runtime.block_on(task).map_err(|_| BackendError::ServiceUnavailable)?;
		}
		if let Some(runtime) = self.runtime.take() {
			runtime.shutdown_timeout(Duration::from_secs(2));
		}
		result.map_err(BackendError::Action)
	}

	fn request<T>(
		&self,
		make: impl FnOnce(oneshot::Sender<T>) -> ServiceRequest,
	) -> BackendResult<T> {
		let sender = self.requests.as_ref().ok_or(BackendError::NotStarted)?;
		let runtime = self.runtime.as_ref().ok_or(BackendError::NotStarted)?;
		let (reply, response) = oneshot::channel();
		sender.try_send(make(reply)).map_err(|_| BackendError::ServiceUnavailable)?;
		runtime.block_on(response).map_err(|_| BackendError::ServiceUnavailable)
	}
}

impl Drop for Backend {
	fn drop(&mut self) { let _ = self.shutdown(); }
}

async fn run_owner(
	mut coordinator: Coordinator<LocalBackend>,
	mut requests: mpsc::Receiver<ServiceRequest>,
	events: mpsc::Sender<StateDelta>,
	initial: Location,
	ready: oneshot::Sender<Result<(), String>>,
) {
	let startup = coordinator.dispatch_command(Command::Navigate(initial)).await;
	match startup {
		Ok(report) => {
			publish(report.deltas, &events);
			let _ = ready.send(Ok(()));
		}
		Err(error) => {
			let _ = ready.send(Err(error.to_string()));
			let _ = coordinator.shutdown().await;
			return;
		}
	}

	loop {
		tokio::select! {
			biased;
			request = requests.recv() => match request {
				Some(ServiceRequest::Dispatch(envelope, reply)) => {
					let result = coordinator.dispatch(envelope).await;
					if let Ok(report) = &result { publish(report.deltas.clone(), &events); }
					let _ = reply.send(result.map(|_| ()).map_err(|error| error.to_string()));
				}
				Some(ServiceRequest::DispatchCurrent(command, reply)) => {
					let result = coordinator.dispatch_command(command).await;
					if let Ok(report) = &result { publish(report.deltas.clone(), &events); }
					let _ = reply.send(result.map(|_| ()).map_err(|error| error.to_string()));
				}
				Some(ServiceRequest::Snapshot(reply)) => { let _ = reply.send(coordinator.snapshot()); }
				Some(ServiceRequest::Shutdown(reply)) => {
					let result = coordinator.shutdown().await.map_err(|error| error.to_string());
					let _ = reply.send(result);
					break;
				}
				None => { let _ = coordinator.shutdown().await; break; }
			},
			feedback = coordinator.pump_feedback() => match feedback {
				Ok(report) => publish(report.deltas, &events),
				Err(_) => break,
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	use tempfile::TempDir;
	use thallium_actions::Command;
	use thallium_fs_core::NativePath;

	use crate::{Backend, BackendConfig};

	#[test]
	fn starts_at_an_explicit_location_and_navigates_without_cwd() {
		let root = TempDir::new().expect("temporary root");
		fs::write(root.path().join("visible.txt"), b"hello").expect("seed file");
		fs::create_dir(root.path().join("child")).expect("seed child");
		let mut backend = Backend::start(BackendConfig {
			initial_location: Some(NativePath::from(root.path())),
			..BackendConfig::default()
		})
		.expect("start backend");

		let initial = backend.snapshot().expect("initial snapshot");
		assert_eq!(initial.directory.entries.len(), 2);
		assert!(initial.directory.entries.iter().any(|entry| entry.name.bytes == b"visible.txt"));

		backend
			.submit(Command::Navigate(thallium_fs_core::Location::local(NativePath::from(
				root.path().join("child"),
			))))
			.expect("navigate");
		let child = backend.snapshot().expect("child snapshot");
		assert_eq!(
			child.current_location.expect("location").path.bytes,
			root.path().join("child").as_os_str().as_encoded_bytes()
		);
		backend.shutdown().expect("shutdown");
	}

	#[test]
	fn rejects_a_relative_initial_location() {
		let result = Backend::start(BackendConfig {
			initial_location: Some(NativePath::from(&b"relative"[..])),
			..BackendConfig::default()
		});
		assert!(result.is_err());
	}
}
