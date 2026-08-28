//! Synchronous, state-only command validation.

use std::fmt;

use thallium_fs_core::{EntryId, StateRevision};
use thallium_protocol::TaskId;

use crate::ValidatedAction;

pub type ValidationResult = Result<ValidatedAction, ValidationError>;

/// Stable reasons a command may be rejected before effects begin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
	StaleRevision { expected: StateRevision, actual: StateRevision },
	InvalidLocation,
	NoCurrentLocation,
	NoHistory,
	UnknownEntry(EntryId),
	UnknownTask(TaskId),
	EmptyTargets,
	TaskNotCancellable(TaskId),
	TaskNotTerminal(TaskId),
	UnsupportedOperation,
}

impl fmt::Display for ValidationError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::StaleRevision { expected, actual } => write!(
				formatter,
				"command targeted state revision {}, but the current revision is {}",
				expected.0, actual.0
			),
			Self::InvalidLocation => formatter.write_str("all filesystem locations must be absolute"),
			Self::NoCurrentLocation => formatter.write_str("no directory is currently open"),
			Self::NoHistory => formatter.write_str("no navigation history is available"),
			Self::UnknownEntry(id) => write!(formatter, "entry {} is not in the current directory", id.0),
			Self::UnknownTask(id) => write!(formatter, "task {} is unknown", id.0),
			Self::EmptyTargets => formatter.write_str("at least one source or target is required"),
			Self::TaskNotCancellable(id) => write!(formatter, "task {} cannot be cancelled", id.0),
			Self::TaskNotTerminal(id) => write!(formatter, "task {} is not finished", id.0),
			Self::UnsupportedOperation => formatter.write_str("the requested operation is unsupported"),
		}
	}
}

impl std::error::Error for ValidationError {}
