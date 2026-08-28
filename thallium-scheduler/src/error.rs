//! Scheduler-level admission and lifecycle failures.

use std::fmt;

use thallium_protocol::TaskId;

/// Failure returned while controlling the scheduler itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerError {
	TaskNotFound(TaskId),
	InvalidInput(String),
	QueueFull,
	Unavailable,
}

impl fmt::Display for SchedulerError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::TaskNotFound(id) => write!(formatter, "task {} was not found", id.0),
			Self::InvalidInput(message) => formatter.write_str(message),
			Self::QueueFull => formatter.write_str("the bounded scheduler queue is full"),
			Self::Unavailable => formatter.write_str("the scheduler is unavailable"),
		}
	}
}

impl std::error::Error for SchedulerError {}

/// Scheduler-domain result alias.
pub type SchedulerResult<T> = Result<T, SchedulerError>;
