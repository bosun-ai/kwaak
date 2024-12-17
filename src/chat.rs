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
}

impl Chat {
    pub(crate) fn add_message(&mut self, message: ChatMessage) {
        if !message.role().is_user() {
            self.new_message_count += 1;
        }

        // If it's a completed tool call, just register it is done and do not add the message
        // The state is updated when rendering on the initial tool call
        if message.role().is_tool() {
            let tool_call_id = message
                .maybe_completed_tool_call()
                .expect("Expected tool call")
                .id();
            self.completed_tool_call_ids
                .insert(tool_call_id.to_string());

            return;
        }
        self.messages.push(message);
    }

    pub fn transition(&mut self, state: ChatState) {
        self.state = state;
    }

    pub fn is_loading(&self) -> bool {
        matches!(
            self.state,
            ChatState::Loading | ChatState::LoadingWithMessage(_)
        )
    }

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
        }
    }
}
