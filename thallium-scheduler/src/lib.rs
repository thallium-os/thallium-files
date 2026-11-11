//! Bounded, renderer-neutral task scheduling for the Thallium 81 backend.
//!
//! The module split follows `PHASE2-PLAN.md` section 3. The aggregate task,
//! progress, cancellation, and leaf-work shapes are informed by
//! `SCHEDULER-RESEARCH.md` sections 1-5 without reusing Yazi code.

pub mod error;
pub mod event;
pub mod handle;
pub mod input;
pub mod ongoing;
pub mod progress;
pub mod queue;
pub mod scheduler;
pub mod status;
pub mod task;

pub use error::{SchedulerError, SchedulerResult};
pub use event::{CleanupOutcome, SchedulerEvent, WorkerEvent};
pub use handle::TaskHandle;
pub use input::{CopyTask, CreateTask, DeleteTask, LinkTask, MoveTask, RenameTask, TaskInput, TaskOptions, TrashTask};
pub use ongoing::Ongoing;
pub use progress::{ActiveWork, AggregateProgress, WorkPhase};
pub use queue::{Priority, QueuedWork, ResourceClass, StableQueue, WorkItem};
pub use scheduler::{Scheduler, SchedulerConfig};
pub use status::{TaskOutcome, TaskStatus};
pub use task::{Task, TaskSpec};
