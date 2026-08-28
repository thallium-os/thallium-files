//! Validated internal actions selected before state or service dispatch.

use thallium_fs_core::StateRevision;
use thallium_protocol::TaskId;
use thallium_state::{StateAction, StateEffect};

/// Internal work selected by command validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
	Reduce(Box<StateAction>),
	Invoke(StateEffect),
	CancelTask(TaskId),
	DismissTask(TaskId),
}

/// Validation output carrying the revision used for the decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedAction {
	pub validated_at: StateRevision,
	pub action:       Action,
}
