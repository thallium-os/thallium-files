//! Semantic workflow coordination for the Thallium 81 backend.
//!
//! This crate is the sole bridge between pure state reduction and asynchronous
//! VFS, scheduler, and watcher effects. The split follows `PHASE2-PLAN.md`
//! sections 1 and 4 and is informed by `WATCHER-STATE-RESEARCH.md` sections
//! 2-3.

pub mod action;
pub mod command;
pub mod coordinator;
pub mod feedback;
pub mod validation;

pub use action::{Action, ValidatedAction};
pub use command::{Command, CommandEnvelope};
pub use coordinator::{ActionError, Coordinator, CoordinatorConfig, DispatchReport, FeedbackReport};
pub use feedback::{EffectError, EffectFeedback, EffectKind, EffectResult};
pub use validation::{ValidationError, ValidationResult};
