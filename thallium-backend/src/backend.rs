//! Backend lifecycle and frontend-neutral façade.
//!
//! `PHASE2-PLAN.md` sections 1, 4, and 5 require a single action/state owner,
//! private runtime machinery, and an owned snapshot/delta surface.

use std::{sync::Arc, time::Duration};

use thallium_actions::{Command, CommandEnvelope, Coordinator, CoordinatorConfig};
use thallium_fs_core::{Location, NativePath};
use thallium_protocol::{StateDelta, StateSnapshot};
use thallium_rclone::{Rclone, Remotes, RoutedFs, config::{Provider, Session}};
use thallium_scheduler::Scheduler;
use thallium_state::BrowserState;
use thallium_watcher::Watcher;
use tokio::{runtime::Runtime, sync::{mpsc, oneshot}, task::JoinHandle};

use crate::{BackendConfig, BackendError, BackendResult, services::{ServiceRequest, publish}};

/// Composition root for the backend runtime and its single service graph.
pub struct Backend {
	runtime:    Option<Runtime>,
	requests:   Option<mpsc::Sender<ServiceRequest>>,
	events:     mpsc::Receiver<StateDelta>,
	owner_task: Option<JoinHandle<()>>,
	/// The remote services this session found configured. Shared with the
	/// router, so a remote configured from the interface is reachable at once.
	remotes:    Remotes,
	/// The rclone daemon, absent when rclone is not installed.
	daemon:     Option<Arc<Rclone>>,
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
		let mut remotes = Remotes::default();
		let mut daemon = None;
		let owner_task = runtime.block_on(async {
			// Remote services are optional: without rclone this is exactly the
			// local filesystem, and the browser is unaffected.
			let remoting = thallium_rclone::connect().await;
			remotes = remoting.remotes;
			daemon = remoting.daemon;
			let vfs = Arc::new(remoting.router);
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
			remotes,
			daemon,
		})
	}

	/// The remote services available this session.
	pub fn remotes(&self) -> &Remotes { &self.remotes }

	/// Whether remotes can be configured at all, which is to say whether
	/// rclone is installed.
	pub fn can_configure_remotes(&self) -> bool { self.daemon.is_some() }

	/// Every storage system this rclone build supports.
	pub fn remote_providers(&self) -> BackendResult<Vec<Provider>> {
		self.on_daemon(|rclone| async move { thallium_rclone::config::providers(&rclone).await })
	}

	/// Start configuring a remote, returning its first question.
	pub fn begin_remote(&self, name: &str, kind: &str) -> BackendResult<Session> {
		let (name, kind) = (name.to_owned(), kind.to_owned());
		self.on_daemon(
			|rclone| async move { thallium_rclone::config::begin(&rclone, &name, &kind).await },
		)
	}

	/// Answer the outstanding question and return what rclone asks next.
	pub fn answer_remote(&self, session: &Session, response: &str) -> BackendResult<Session> {
		let (session, response) = (session.clone(), response.to_owned());
		self.on_daemon(|rclone| async move {
			thallium_rclone::config::answer(&rclone, &session, &response).await
		})
	}

	/// Remove a remote from the user's rclone configuration.
	pub fn forget_remote(&self, name: &str) -> BackendResult<()> {
		let name = name.to_owned();
		self.on_daemon(|rclone| async move { thallium_rclone::config::delete(&rclone, &name).await })
	}

	/// Re-read the configuration, so a remote just added becomes navigable.
	pub fn refresh_remotes(&self) -> BackendResult<()> {
		let names = self.on_daemon(|rclone| async move { rclone.remotes().await })?;
		self.remotes.replace(names);
		Ok(())
	}

	/// Run one daemon call on the owned runtime.
	/// Start a browser consent flow for a provider that needs OAuth.
	///
	/// The handle it returns is filled in by a child process, so the caller can
	/// keep drawing while the user logs in; see [`thallium_rclone::authorize`].
	pub fn begin_consent(&self, kind: &str) -> BackendResult<thallium_rclone::authorize::Shared> {
		let runtime = self.runtime.as_ref().ok_or(BackendError::NotStarted)?;
		let _guard = runtime.enter();
		Ok(thallium_rclone::authorize::start(kind))
	}

	fn on_daemon<T, F>(&self, call: impl FnOnce(Arc<Rclone>) -> F) -> BackendResult<T>
	where
		F: Future<Output = thallium_fs_core::FsResult<T>>,
	{
		let rclone = self.daemon.clone().ok_or_else(|| {
			BackendError::Configuration("rclone is not installed, so remotes cannot be configured".into())
		})?;
		let runtime = self.runtime.as_ref().ok_or(BackendError::NotStarted)?;
		runtime.block_on(call(rclone)).map_err(|error| BackendError::Action(error.message))
	}

	/// Validate and enqueue a semantic command for the single action/state owner.
	pub fn submit_command(&mut self, command: CommandEnvelope) -> BackendResult<()> {
		self.request(|reply| ServiceRequest::Dispatch(command, reply))?.map_err(BackendError::Action)
	}

	/// Dispatch without waiting for the outcome.
	///
	/// The owner does not answer a dispatch until its effects have run, and a
	/// remote listing is a network round trip. Waiting for that on the thread
	/// that draws is what made opening a remote look like a hang, so the reply
	/// is handed back for the caller to collect on a later poll.
	pub fn submit_detached(
		&mut self,
		command: Command,
	) -> BackendResult<oneshot::Receiver<Result<(), String>>> {
		let sender = self.requests.as_ref().ok_or(BackendError::NotStarted)?;
		let (reply, response) = oneshot::channel();
		sender
			.try_send(ServiceRequest::DispatchCurrent(command, reply))
			.map_err(|_| BackendError::ServiceUnavailable)?;
		Ok(response)
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
	mut coordinator: Coordinator<RoutedFs>,
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
