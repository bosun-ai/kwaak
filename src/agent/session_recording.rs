use serde::{Deserialize, Serialize};
use swiftide::chat_completion::ChatMessage;

use crate::repository::Repository;

use super::session::Session;

// A session is recording represents a session at a given point in time
//
// Use cases:
// - Session replay
// - Session persistance
// - Testing, evaluation and benchmarking
//
// A session should be able to create a recording at any point in time.
//
// A session can also be resumed from a recording.
//
// TODO:
// - Needs latest Swiftide for deriving Serialize and Deserialize
// - How are we going to deal multiple agents? (Move active agent to session and deserialize that?)
//   > Maybe a session should be a trait instead
// - Should the recording get the diff, or should it be created with the diff?
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecording {
    chat_messages: Vec<ChatMessage>,
    diff: Option<String>,
    repository: Repository,
}
