//! Errors surfaced by the composition façade.
//!
//! Per `PHASE2-PLAN.md` section 5, concrete Tokio senders, join failures, and
//! platform error objects remain private and will later map to protocol DTOs.

/// Result returned by backend lifecycle and façade operations.
pub type BackendResult<T> = Result<T, BackendError>;

/// Coarse errors at the backend composition boundary.
#[derive(Debug)]
pub enum BackendError {
	/// Startup was requested for an already-running backend.
	AlreadyStarted,
	/// An operation requires services that have not been started.
	NotStarted,
	/// The private action or event port is no longer available.
	ServiceUnavailable,
	/// Constructing the owned async runtime failed.
	Runtime(std::io::Error),
	/// Startup configuration or the requested initial location is invalid.
	Configuration(String),
	/// A semantic command or asynchronous effect failed.
	Action(String),
}

impl From<std::io::Error> for BackendError {
	fn from(error: std::io::Error) -> Self { Self::Runtime(error) }
}

impl std::fmt::Display for BackendError {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::AlreadyStarted => formatter.write_str("the backend is already started"),
			Self::NotStarted => formatter.write_str("the backend is not started"),
			Self::ServiceUnavailable => formatter.write_str("the backend service is unavailable or busy"),
			Self::Runtime(error) => write!(formatter, "failed to create the async runtime: {error}"),
			Self::Configuration(message) | Self::Action(message) => formatter.write_str(message),
		}
	}
}

impl std::error::Error for BackendError {}
