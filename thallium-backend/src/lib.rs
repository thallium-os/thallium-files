//! Composition root and public façade for the Thallium 81 backend.
//!
//! The ownership and boundary shape follows `PHASE2-PLAN.md` sections 1 and 5.
//! Runtime handles and mutable service internals stay behind this crate; a Qt
//! adapter receives only owned protocol values. All runtime and service
//! ownership stays behind synchronous, bounded frontend-neutral methods.

mod backend;
mod config;
mod error;
mod search;
mod services;

#[cfg(feature = "qt")]
pub mod qt_bridge;

pub use backend::Backend;
pub use config::BackendConfig;
pub use error::{BackendError, BackendResult};
