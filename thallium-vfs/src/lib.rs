//! Renderer-neutral asynchronous filesystem boundary for Thallium 81.
//!
//! The shape follows `FS-VFS-RESEARCH.md` section 2 and the filesystem/VFS
//! boundary in `PHASE2-PLAN.md`. This crate owns no browser state and every
//! operation receives an explicit location.

pub mod capabilities;
pub mod copy;
pub mod facade;
pub mod local;
pub mod traits;

pub use capabilities::Capabilities;
pub use copy::{MoveOptions, MoveOutcome, StagedCopyOptions, StagedCopyOutcome};
pub use facade::{CreateDirOptions, DeleteOptions, FileSystem, OpenWriteOptions, StatOptions};
pub use local::LocalBackend;
pub use traits::DirectoryReader;
