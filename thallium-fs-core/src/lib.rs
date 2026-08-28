//! Owned filesystem-domain values for the Thallium 81 backend.
//!
//! This crate is intentionally synchronous and renderer-neutral. Its boundary
//! is defined by `PHASE2-PLAN.md` section 1 and its shapes are informed by
//! `FS-VFS-RESEARCH.md` section 1.

pub mod attrs;
pub mod delta;
pub mod error;
pub mod id;
pub mod location;
pub mod metadata;
pub mod path;
pub mod policy;
pub mod signature;

pub use attrs::FileAttrs;
pub use delta::{FsChange, FsDelta};
pub use error::{FsError, FsErrorCode, FsResult};
pub use id::{EntryId, ListingGeneration, StateRevision};
pub use location::{BackendId, Location};
pub use metadata::{EntryMetadata, FileEntry, FileKind, SymlinkInfo, Timestamp};
pub use path::NativePath;
pub use policy::ConflictPolicy;
pub use signature::MetadataSignature;
