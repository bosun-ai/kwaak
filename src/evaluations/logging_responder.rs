use crate::commands::{CommandResponse, Responder};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use swiftide::chat_completion::ChatMessage;

#[derive(Debug, Clone)]
pub struct LoggingResponder {
    messages: Arc<Mutex<Vec<String>>>,
}

impl LoggingResponder {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
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

#[async_trait]
impl Responder for LoggingResponder {
    async fn agent_message(&self, message: ChatMessage) {
        let mut messages = self.messages.lock().unwrap();
        let formatted = format!("{message:?}").replace("\\\\", "\\");
        messages.push(format!(
            "DEBUG: Agent message: {}",
            Self::format_string(&formatted)
        ));
    }

    async fn update(&self, message: &str) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(format!(
            "DEBUG: State update: {}",
            Self::format_string(message)
        ));
    }

    async fn send(&self, response: CommandResponse) {
        let mut messages = self.messages.lock().unwrap();
        let formatted = format!("{response:?}").replace("\\\\", "\\");
        messages.push(format!(
            "DEBUG: Command response: {}",
            Self::format_string(&formatted)
        ));
    }

    async fn system_message(&self, message: &str) {
        let mut messages = self.messages.lock().unwrap();
        messages.push(format!(
            "DEBUG: System message: {}",
            Self::format_string(message)
        ));
    }
}
