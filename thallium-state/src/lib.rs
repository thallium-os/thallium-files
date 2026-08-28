//! Renderer-neutral single-pane browser state and pure transitions.
//!
//! The single-owner, split-phase shape follows `PHASE2-PLAN.md` section 4 and
//! `WATCHER-STATE-RESEARCH.md` sections 2-3. This crate contains no renderer,
//! async runtime, channel, callback, I/O, or global mutable state.

pub mod action;
pub mod browser;
pub mod directory;
pub mod effect;
pub mod history;
pub mod reducer;
pub mod selection;

pub use action::StateAction;
pub use browser::BrowserState;
pub use directory::DirectoryState;
pub use effect::{StateEffect, TaskRequest};
pub use history::NavigationHistory;
pub use reducer::{Reduction, reduce};
pub use selection::SelectionState;
