//! Configuring remotes without leaving the application.
//!
//! rclone's own `rclone config` is a question-and-answer loop, and the
//! remote-control API exposes that same loop non-interactively: each call
//! returns either the next question or the fact that it is finished. Driving
//! it, rather than reimplementing it, is what keeps this provider-agnostic —
//! Google Drive's browser consent and an SFTP password arrive through exactly
//! the same states, and a provider rclone adds later needs no code here.

use serde_json::{Value, json};
use thallium_fs_core::{FsError, FsErrorCode, FsResult};

use crate::rc::Rclone;

/// A storage system rclone can talk to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Provider {
	/// The `type` written into the configuration, for example `drive`.
	pub kind:        String,
	/// Human wording, for example `Google Drive`.
	pub description: String,
}

/// One suggested answer offered alongside a question.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Choice {
	pub value: String,
	pub help:  String,
}

/// A question rclone is waiting on.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Question {
	pub name:     String,
	/// rclone's own explanation. For a provider that needs browser consent
	/// this is where the authorisation link arrives, so it is shown verbatim.
	pub help:     String,
	pub default:  String,
	pub choices:  Vec<Choice>,
	/// Whether the answer should be masked and stored obscured.
	pub secret:   bool,
	pub required: bool,
}

/// Where a configuration conversation currently stands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Step {
	/// rclone is waiting for this answer.
	Ask(Question),
	/// The remote now exists in the user's rclone configuration.
	Done,
	/// rclone refused; the message is its own.
	Failed(String),
}

/// One in-progress configuration conversation.
#[derive(Clone, Debug)]
pub struct Session {
	name:  String,
	kind:  String,
	/// rclone's opaque continuation token. Empty before the first answer.
	state: String,
	step:  Step,
}

impl Session {
	#[must_use]
	pub fn name(&self) -> &str { &self.name }

	#[must_use]
	pub fn kind(&self) -> &str { &self.kind }

	#[must_use]
	pub fn step(&self) -> &Step { &self.step }

	/// Whether this conversation has stopped, either way.
	#[must_use]
	pub fn is_finished(&self) -> bool { !matches!(self.step, Step::Ask(_)) }

	/// The question waiting to be answered, if the conversation is still open.
	#[must_use]
	pub fn question(&self) -> Option<&Question> {
		match &self.step {
			Step::Ask(question) => Some(question),
			_ => None,
		}
	}

	/// Whether rclone is waiting for an OAuth token rather than a plain answer.
	/// The frontend runs [`crate::authorize`] instead of asking the user.
	#[must_use]
	pub fn wants_token(&self) -> bool {
		self.question().is_some_and(|question| question.name == "config_token")
	}
}

/// Every storage system this rclone build supports, in alphabetical order.
pub async fn providers(rclone: &Rclone) -> FsResult<Vec<Provider>> {
	let payload = rclone.call("config/providers", json!({})).await?;
	let listed = payload
		.get("providers")
		.and_then(Value::as_array)
		.ok_or_else(|| FsError::new(FsErrorCode::Io, "rclone listed no storage providers", None))?;
	let mut providers: Vec<Provider> = listed
		.iter()
		.filter_map(|provider| {
			let kind = provider.get("Name").and_then(Value::as_str)?.to_owned();
			let description =
				provider.get("Description").and_then(Value::as_str).unwrap_or(&kind).to_owned();
			Some(Provider { kind, description })
		})
		.collect();
	providers.sort_by(|left, right| left.description.cmp(&right.description));
	Ok(providers)
}

/// Start configuring a new remote and return its first question.
///
/// rclone writes the remote to the configuration on this first call, before a
/// single question has been answered, so an abandoned conversation has to be
/// cleaned up with [`delete`] rather than simply dropped.
pub async fn begin(rclone: &Rclone, name: &str, kind: &str) -> FsResult<Session> {
	if name.is_empty() {
		return Err(FsError::new(FsErrorCode::InvalidInput, "a remote needs a name", None));
	}
	if name.contains(':') || name.contains('/') {
		return Err(FsError::new(
			FsErrorCode::InvalidInput,
			"a remote name cannot contain ':' or '/'",
			None,
		));
	}
	let mut session = Session {
		name:  name.to_owned(),
		kind:  kind.to_owned(),
		state: String::new(),
		step:  Step::Done,
	};
	let payload = rclone
		.call(
			"config/create",
			json!({
				"name": name,
				"type": kind,
				"parameters": {},
				// `all` is what makes rclone ask anything: without it a
				// non-interactive create writes the remote with defaults and
				// returns as finished, having asked nothing at all.
				"opt": { "nonInteractive": true, "obscure": true, "all": true },
			}),
		)
		.await?;
	session.absorb(&payload);
	skip_locality(rclone, &mut session).await?;
	Ok(session)
}

/// rclone asks whether a browser is available here. Answering yes makes the
/// next `config/create` call run the whole consent flow before it returns,
/// which would hold the interface still for as long as the user takes to log
/// in. The headless branch returns at once and asks for a token instead, and
/// [`crate::authorize`] is how that token is obtained without sending the user
/// to a documentation page. So the question is never put to the user.
async fn skip_locality(rclone: &Rclone, session: &mut Session) -> FsResult<()> {
	// Bounded because the loop is driven by what rclone sends back: were it ever
	// to keep asking the same question, this would otherwise never return.
	for _ in 0..4 {
		if !session.question().is_some_and(|question| question.name == "config_is_local") {
			return Ok(());
		}
		*session = ask(rclone, session, "false").await?;
	}
	Ok(())
}

/// Answer the outstanding question and return whatever rclone asks next.
pub async fn answer(rclone: &Rclone, session: &Session, response: &str) -> FsResult<Session> {
	if session.is_finished() {
		return Ok(session.clone());
	}
	let mut next = ask(rclone, session, response).await?;
	skip_locality(rclone, &mut next).await?;
	Ok(next)
}

/// One turn of the conversation, with no policy of its own.
async fn ask(rclone: &Rclone, session: &Session, response: &str) -> FsResult<Session> {
	let mut next = session.clone();
	let payload = rclone
		.call(
			"config/create",
			json!({
				"name": session.name,
				"type": session.kind,
				"parameters": {},
				"opt": {
					"nonInteractive": true,
					"obscure": true,
					"all": true,
					"continue": true,
					"state": session.state,
					"result": response,
				},
			}),
		)
		.await?;
	next.absorb(&payload);
	Ok(next)
}

/// Remove a remote from the user's rclone configuration.
///
/// Also how an abandoned [`begin`] is undone; see its note.
pub async fn delete(rclone: &Rclone, name: &str) -> FsResult<()> {
	rclone.call("config/delete", json!({ "name": name })).await?;
	Ok(())
}

impl Session {
	/// Read one `ConfigOut` response into this conversation.
	fn absorb(&mut self, payload: &Value) {
		let error = payload.get("Error").and_then(Value::as_str).unwrap_or_default();
		if !error.is_empty() {
			self.step = Step::Failed(error.to_owned());
			return;
		}
		self.state = payload.get("State").and_then(Value::as_str).unwrap_or_default().to_owned();
		let option = payload.get("Option").filter(|option| !option.is_null());
		self.step = match option {
			// No question and no state left: the remote has been written.
			None => Step::Done,
			Some(option) => Step::Ask(question_from(option)),
		};
	}
}

fn question_from(option: &Value) -> Question {
	let choices = option
		.get("Examples")
		.and_then(Value::as_array)
		.map(|examples| {
			examples
				.iter()
				.filter_map(|example| {
					Some(Choice {
						value: example.get("Value").and_then(Value::as_str)?.to_owned(),
						help:  example.get("Help").and_then(Value::as_str).unwrap_or_default().to_owned(),
					})
				})
				.collect()
		})
		.unwrap_or_default();

	Question {
		name: option.get("Name").and_then(Value::as_str).unwrap_or_default().to_owned(),
		help: option.get("Help").and_then(Value::as_str).unwrap_or_default().to_owned(),
		default: scalar(option.get("Default")),
		choices,
		secret: option.get("IsPassword").and_then(Value::as_bool).unwrap_or(false),
		required: option.get("Required").and_then(Value::as_bool).unwrap_or(false),
	}
}

/// Render a default of whatever JSON type rclone used for it.
fn scalar(value: Option<&Value>) -> String {
	match value {
		Some(Value::String(text)) => text.clone(),
		Some(Value::Bool(flag)) => flag.to_string(),
		Some(Value::Number(number)) => number.to_string(),
		_ => String::new(),
	}
}

/// The first URL in a question's help text, when there is one.
///
/// A provider that needs browser consent puts its authorisation link in the
/// help; pulling it out lets the frontend offer it as a link rather than
/// asking the user to find it in a paragraph.
#[must_use]
pub fn link_in(help: &str) -> Option<String> {
	let start = help.find("http://").or_else(|| help.find("https://"))?;
	let rest = &help[start..];
	let end = rest.find(|character: char| character.is_whitespace()).unwrap_or(rest.len());
	Some(rest[..end].trim_end_matches(['.', ',', ')']).to_owned())
}

/// Turn a transport failure into the same shape a refusal has.
#[must_use]
pub fn failure(error: &FsError) -> Step { Step::Failed(error.message.clone()) }

#[cfg(test)]
mod tests {
	use serde_json::json;

	use super::{Session, Step, link_in, question_from};

	fn session() -> Session {
		Session {
			name:  "gdrive".to_owned(),
			kind:  "drive".to_owned(),
			state: String::new(),
			step:  Step::Done,
		}
	}

	#[test]
	fn a_question_carries_its_choices_and_default() {
		let question = question_from(&json!({
			"Name": "scope",
			"Help": "Scope that rclone should use.",
			"Default": "drive",
			"Required": true,
			"IsPassword": false,
			"Examples": [
				{ "Value": "drive", "Help": "Full access" },
				{ "Value": "drive.readonly", "Help": "Read-only access" },
			],
		}));
		assert_eq!(question.name, "scope");
		assert_eq!(question.default, "drive");
		assert!(question.required);
		assert_eq!(question.choices.len(), 2);
		assert_eq!(question.choices[1].value, "drive.readonly");
	}

	#[test]
	fn non_string_defaults_are_still_rendered() {
		assert_eq!(question_from(&json!({ "Default": true })).default, "true");
		assert_eq!(question_from(&json!({ "Default": 42 })).default, "42");
		assert_eq!(question_from(&json!({})).default, "");
	}

	#[test]
	fn a_state_and_option_become_the_next_question() {
		let mut session = session();
		session.absorb(&json!({
			"State": "*oauth",
			"Option": { "Name": "config_token", "Help": "Paste the token here", "IsPassword": true },
			"Error": "",
		}));
		assert_eq!(session.state, "*oauth");
		match session.step() {
			Step::Ask(question) => {
				assert_eq!(question.name, "config_token");
				assert!(question.secret);
			}
			other => panic!("expected a question, got {other:?}"),
		}
		assert!(!session.is_finished());
	}

	#[test]
	fn no_option_means_the_remote_was_written() {
		let mut session = session();
		session.absorb(&json!({ "State": "", "Option": null, "Error": "" }));
		assert_eq!(session.step(), &Step::Done);
		assert!(session.is_finished());
	}

	#[test]
	fn an_error_ends_the_conversation_with_rclones_own_words() {
		let mut session = session();
		session.absorb(&json!({ "Error": "didn't find backend called \"nope\"" }));
		assert_eq!(session.step(), &Step::Failed("didn't find backend called \"nope\"".to_owned()));
		assert!(session.is_finished());
	}

	#[test]
	fn an_authorisation_link_is_found_in_help_text() {
		let help =
			"Go to the following link: https://accounts.google.com/o/oauth2/auth?x=1 then paste.";
		assert_eq!(link_in(help).as_deref(), Some("https://accounts.google.com/o/oauth2/auth?x=1"));
		assert_eq!(link_in("no link here"), None);
	}
}
