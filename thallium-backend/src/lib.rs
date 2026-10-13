//! Composition root and public façade for the Thallium 81 backend.
//!
//! The ownership and boundary shape follows `PHASE2-PLAN.md` sections 1 and 5.
//! Runtime handles and mutable service internals stay behind this crate; a Qt
//! adapter receives only owned protocol values. All runtime and service
//! ownership stays behind synchronous, bounded frontend-neutral methods.

mod backend;
mod config;
mod error;
// Only the Qt bridge calls these; built without it, they still compile so
// their tests run.
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod apps;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod archive;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod bookmarks;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod bulk_rename;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod drives;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod files_theme;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod properties;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod recents;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod search;
mod services;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod storage;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod trash_bin;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod uca;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod undo;
#[cfg_attr(not(feature = "qt"), allow(dead_code))]
mod uri;

#[cfg(feature = "qt")]
pub mod qt_bridge;

pub use backend::Backend;
pub use config::BackendConfig;
pub use error::{BackendError, BackendResult};
