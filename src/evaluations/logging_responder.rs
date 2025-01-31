use crate::commands::{CommandResponse, Responder};
use std::sync::Mutex;
use swiftide::chat_completion::ChatMessage;

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

    fn format_string(s: &str) -> String {
        s.replace("\\\"", "\"")
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\r", "\r")
    }
}

impl Responder for LoggingResponder {
    fn agent_message(&self, message: ChatMessage) {
        let mut messages = self.messages.lock().unwrap();
        let formatted = format!("{message:?}").replace("\\\\", "\\");
        messages.push(format!(
            "DEBUG: Agent message: {}",
            Self::format_string(&formatted)
        ));
    }

    fn update(&self, message: &str) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(format!(
            "DEBUG: State update: {}",
            Self::format_string(message)
        ));
    }

    fn send(&self, response: CommandResponse) {
        let mut messages = self.messages.lock().unwrap();
        let formatted = format!("{response:?}").replace("\\\\", "\\");
        messages.push(format!(
            "DEBUG: Command response: {}",
            Self::format_string(&formatted)
        ));
    }

    fn system_message(&self, message: &str) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(format!(
            "DEBUG: System message: {}",
            Self::format_string(message)
        ));
    }

    fn rename_chat(&self, _name: &str) {}
    fn rename_branch(&self, _name: &str) {}
}
