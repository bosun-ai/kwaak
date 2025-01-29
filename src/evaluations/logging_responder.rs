use std::sync::Mutex;
use swiftide::chat_completion::ChatMessage;
use crate::commands::{CommandResponse, Responder};

#[derive(Debug)]
pub struct LoggingResponder {
    messages: Mutex<Vec<String>>,
}

impl LoggingResponder {
    pub fn new() -> Self {
        Self {
            messages: Mutex::new(Vec::new()),
        }
    }

    pub fn get_log(&self) -> String {
        self.messages.lock().unwrap().join("\n")
    }
}

impl Responder for LoggingResponder {
    fn agent_message(&self, message: ChatMessage) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(format!("DEBUG: Agent message: {message:?}"));
    }

    fn update(&self, message: &str) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(format!("DEBUG: State update: {message}"));
    }

    fn send(&self, response: CommandResponse) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(format!("DEBUG: Command response: {response:?}"));
    }

    fn system_message(&self, message: &str) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(format!("DEBUG: System message: {message}"));
    }

    fn rename_chat(&self, _name: &str) {}
    fn rename_branch(&self, _name: &str) {}
}
