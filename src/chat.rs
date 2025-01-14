use std::collections::HashSet;

use ratatui::widgets::ScrollbarState;

use crate::chat_message::ChatMessage;

#[derive(Debug, Clone)]
pub struct Chat {
    /// Display name of the chat
    pub name: String,
    /// Identifier used to match responses
    pub uuid: uuid::Uuid,
    pub messages: Vec<ChatMessage>,
    pub state: ChatState,
    pub new_message_count: usize,
    pub completed_tool_call_ids: HashSet<String>,

    // Scrolling is per chat
    // but handled in the ui
    pub vertical_scroll_state: ScrollbarState,
    pub vertical_scroll: usize,
    pub num_lines: usize,
}

impl Chat {
    pub fn add_message(&mut self, message: ChatMessage) {
        if !message.role().is_user() {
            self.new_message_count += 1;
        }

        // If it's a completed tool call, just register it is done and do not add the message
        // The state is updated when rendering on the initial tool call
        if message.role().is_tool() {
            let Some(tool_call) = message.maybe_completed_tool_call() else {
                tracing::error!(
                    "Received a tool message without a tool call ID: {:?}",
                    message
                );
                return;
            };

            self.completed_tool_call_ids
                .insert(tool_call.id().to_string());

            return;
        }
        self.messages.push(message);
    }

    pub fn transition(&mut self, state: ChatState) {
        self.state = state;
    }

    #[must_use]
    pub fn is_loading(&self) -> bool {
        matches!(
            self.state,
            ChatState::Loading | ChatState::LoadingWithMessage(_)
        )
    }

    #[must_use]
    pub fn is_tool_call_completed(&self, tool_call_id: &str) -> bool {
        self.completed_tool_call_ids.contains(tool_call_id)
    }
}

#[derive(Debug, Clone, Default, strum::EnumIs, PartialEq)]
pub enum ChatState {
    Loading,
    LoadingWithMessage(String),
    #[default]
    Ready,
}

impl Default for Chat {
    fn default() -> Self {
        Self {
            name: "Chat".to_string(),
            uuid: uuid::Uuid::new_v4(),
            messages: Vec::new(),
            state: ChatState::default(),
            new_message_count: 0,
            completed_tool_call_ids: HashSet::new(),
            vertical_scroll_state: ScrollbarState::default(),
            vertical_scroll: 0,
            num_lines: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use swiftide::chat_completion;

    use super::*;
    use crate::chat_message::ChatMessage;

    #[test]
    fn test_add_message_increases_new_message_count() {
        let mut chat = Chat::default();
        let message = ChatMessage::new_system("Test message");

        chat.add_message(message);

        assert_eq!(chat.new_message_count, 1);
        assert_eq!(chat.messages.len(), 1);
    }

    #[test]
    fn test_add_message_does_not_increase_new_message_count_for_user() {
        let mut chat = Chat::default();
        let message = ChatMessage::new_user("Test message");

        chat.add_message(message);

        assert_eq!(chat.new_message_count, 0);
        assert_eq!(chat.messages.len(), 1);
    }

    #[test]
    fn test_add_message_tool_call() {
        let mut chat = Chat::default();
        let tool_call = chat_completion::ToolCall::builder()
            .id("tool_call_id")
            .name("some_tool")
            .build()
            .unwrap();
        let message =
            chat_completion::ChatMessage::new_tool_output(tool_call, String::new()).into();

        chat.add_message(message);

        assert!(chat.is_tool_call_completed("tool_call_id"));
        assert_eq!(chat.messages.len(), 0);
    }

    #[test]
    fn test_transition() {
        let mut chat = Chat::default();
        chat.transition(ChatState::Loading);

        assert!(chat.is_loading());
    }

    #[test]
    fn test_is_loading() {
        let chat = Chat {
            state: ChatState::Loading,
            ..Default::default()
        };

        assert!(chat.is_loading());
    }

    #[test]
    fn test_is_tool_call_completed() {
        let mut chat = Chat::default();
        chat.completed_tool_call_ids
            .insert("tool_call_id".to_string());

        assert!(chat.is_tool_call_completed("tool_call_id"));
    }
}
