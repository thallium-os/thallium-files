//! Stable filesystem failure values.
//!
//! The owned shape follows `PHASE2-PLAN.md` sections 2 and 5 and deliberately
//! carries no runtime-specific error object.

use std::{fmt, io};

use crate::Location;

/// Stable, frontend-safe classification of a filesystem failure.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FsErrorCode {
	/// A requested entry was not found.
	NotFound,
	/// Access was denied.
	PermissionDenied,
	/// An entry already exists.
	AlreadyExists,
	/// An operation crossed filesystem/backend boundaries it cannot cross.
	CrossDevice,
	/// A path component or operation input was invalid.
	InvalidInput,
	/// A non-directory was used where a directory was required.
	NotDirectory,
	/// A directory was used where a non-directory was required.
	IsDirectory,
	/// The target filesystem has insufficient capacity.
	NoSpace,
	/// A directory is not empty.
	DirectoryNotEmpty,
	/// The target filesystem is read-only.
	ReadOnly,
	/// The resource is temporarily busy.
	Busy,
	/// Symbolic-link traversal exceeded the platform limit.
	TooManySymlinks,
	/// A path component exceeds the platform name limit.
	NameTooLong,
	/// An operation was interrupted and may be retried.
	Interrupted,
	/// An operation timed out.
	TimedOut,
	/// The backend does not support the requested capability.
	Unsupported,
	/// The operation was cancelled.
	Cancelled,
	/// An I/O failure not covered by a more stable category.
	Io,
	/// An error category unknown to the current contract.
	Unknown,
}

/// Owned filesystem error suitable for state and protocol DTO conversion.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FsError {
	/// Stable error category.
	pub code:     FsErrorCode,
	/// Human-readable diagnostic text.
	pub message:  String,
	/// Platform error number, when one exists.
	pub os_code:  Option<i32>,
	/// Location most directly associated with the failure.
	pub location: Option<Location>,
}

/// Filesystem-domain result alias.
pub type FsResult<T> = Result<T, FsError>;

impl FsErrorCode {
	/// Classifies an I/O error into the stable filesystem-domain vocabulary.
	#[must_use]
	pub fn from_io_error(error: &io::Error) -> Self {
		if let Some(code) = error.raw_os_error() {
			return match code {
				1 | 13 => Self::PermissionDenied,
				2 => Self::NotFound,
				16 => Self::Busy,
				17 => Self::AlreadyExists,
				18 => Self::CrossDevice,
				20 => Self::NotDirectory,
				21 => Self::IsDirectory,
				22 => Self::InvalidInput,
				28 | 122 => Self::NoSpace,
				30 => Self::ReadOnly,
				36 => Self::NameTooLong,
				39 => Self::DirectoryNotEmpty,
				40 => Self::TooManySymlinks,
				_ => Self::from_error_kind(error.kind()),
			};
		}
		Self::from_error_kind(error.kind())
	}

	fn from_error_kind(kind: io::ErrorKind) -> Self {
		match kind {
			io::ErrorKind::NotFound => Self::NotFound,
			io::ErrorKind::PermissionDenied => Self::PermissionDenied,
			io::ErrorKind::AlreadyExists => Self::AlreadyExists,
			io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => Self::InvalidInput,
			io::ErrorKind::Unsupported => Self::Unsupported,
			io::ErrorKind::Interrupted => Self::Interrupted,
			io::ErrorKind::TimedOut => Self::TimedOut,
			io::ErrorKind::WouldBlock => Self::Busy,
			_ => Self::Io,
		}
	}
}

impl FsError {
	/// Converts an I/O error while retaining an optional affected location.
	#[must_use]
	pub fn from_io(error: io::Error, location: Option<Location>) -> Self {
		Self {
			code: FsErrorCode::from_io_error(&error),
			message: error.to_string(),
			os_code: error.raw_os_error(),
			location,
		}
	}

	/// Creates an error that has no platform error number.
	#[must_use]
	pub fn new(code: FsErrorCode, message: impl Into<String>, location: Option<Location>) -> Self {
		Self { code, message: message.into(), os_code: None, location }
	}

	/// Attaches the location relevant to an error.
	#[must_use]
	pub fn at(mut self, location: Location) -> Self {
		self.location = Some(location);
		self
	}
}

impl From<io::Error> for FsError {
	fn from(error: io::Error) -> Self { Self::from_io(error, None) }
}

impl fmt::Display for FsError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		if let Some(location) = &self.location {
			write!(formatter, "{}: {}", location.path, self.message)
		} else {
			formatter.write_str(&self.message)
		}
	}
}

impl std::error::Error for FsError {}

#[cfg(test)]
mod tests {
	use std::io;

	use super::FsErrorCode;

	#[test]
	fn maps_portable_and_linux_errors() {
		assert_eq!(
			FsErrorCode::from_io_error(&io::Error::from(io::ErrorKind::NotFound)),
			FsErrorCode::NotFound
		);
		assert_eq!(
			FsErrorCode::from_io_error(&io::Error::from_raw_os_error(18)),
			FsErrorCode::CrossDevice
		);
		assert_eq!(FsErrorCode::from_io_error(&io::Error::from_raw_os_error(28)), FsErrorCode::NoSpace);
	}
}
