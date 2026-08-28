//! Composition-root configuration.
//!
//! `PHASE2-PLAN.md` sections 3 and 4 place fixed scheduler and watcher bounds
//! at backend startup rather than in frontend or renderer state.

use thallium_fs_core::NativePath;
use thallium_scheduler::SchedulerConfig;
use thallium_watcher::WatcherConfig;

/// Fixed startup configuration for the backend runtime and owned services.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendConfig {
	/// Number of worker threads in the backend-owned Tokio runtime.
	pub runtime_worker_threads: usize,
	/// Bounded scheduler admission and execution policy.
	pub scheduler:              SchedulerConfig,
	/// Bounded filesystem-notification policy.
	pub watcher:                WatcherConfig,
	/// Capacity of the future action-to-Qt event port.
	pub event_capacity:         usize,
	/// Explicit initial directory. `None` resolves `$HOME` without consulting
	/// the process-global current working directory.
	pub initial_location:       Option<NativePath>,
}

impl Default for BackendConfig {
	fn default() -> Self {
		Self {
			runtime_worker_threads: 2,
			scheduler:              SchedulerConfig::default(),
			watcher:                WatcherConfig::default(),
			event_capacity:         256,
			initial_location:       None,
		}
	}
}
