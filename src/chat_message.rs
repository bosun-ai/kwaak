/// Represents a chat message that can be stored in a [`Chat`]
///
/// Messages are expected to be formatted strings and are displayed as-is. Markdown is rendered
/// using `tui-markdown`.
///
/// TODO: All should be Cows
#[derive(Clone, Default, PartialEq)]
pub struct ChatMessage {
    role: ChatRole,
    formatted_content: String,
    original: Option<swiftide::chat_completion::ChatMessage>,
}

// Debug with truncated content
impl std::fmt::Debug for ChatMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatMessage")
            .field("role", &self.role)
            .field(
                "content",
                &self.formatted_content[..std::cmp::min(10, self.formatted_content.len())]
                    .to_string(),
            )
            .field("original", &self.original)
            .finish()
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    strum::EnumString,
    strum::Display,
    strum::AsRefStr,
    strum::EnumIs,
    PartialEq,
)]
pub enum ChatRole {
    User,
    #[default]
    System,
    Command,
    Assistant,
    Tool,
}

impl ChatMessage {
    pub fn new_user(msg: impl Into<String>) -> ChatMessage {
        ChatMessage::default()
            .with_role(ChatRole::User)
            .with_formatted_content(msg.into())
            .to_owned()
    }

    pub fn new_system(msg: impl Into<String>) -> ChatMessage {
        ChatMessage::default()
            .with_role(ChatRole::System)
            .with_formatted_content(msg.into())
            .to_owned()
    }

    pub fn new_command(cmd: impl Into<String>) -> ChatMessage {
        ChatMessage::default()
            .with_role(ChatRole::Command)
            .with_formatted_content(cmd.into().to_string())
            .to_owned()
    }

    pub fn new_assistant(msg: impl Into<String>) -> ChatMessage {
        ChatMessage::default()
            .with_role(ChatRole::Assistant)
            .with_formatted_content(msg.into())
            .to_owned()
    }

    pub fn new_tool(msg: impl Into<String>) -> ChatMessage {
        ChatMessage::default()
            .with_role(ChatRole::Tool)
            .with_formatted_content(msg.into())
            .to_owned()
    }

    pub fn with_role(&mut self, role: ChatRole) -> &mut Self {
        self.role = role;
        self
    }

    pub fn with_formatted_content(&mut self, content: impl Into<String>) -> &mut Self {
        self.formatted_content = content.into();
        self
    }

    pub fn with_original(&mut self, original: swiftide::chat_completion::ChatMessage) -> &mut Self {
        self.original = Some(original);
        self
    }

    #[must_use]
    pub fn formatted_content(&self) -> &str {
        &self.formatted_content
    }
    #[must_use]
    pub fn role(&self) -> &ChatRole {
        &self.role
    }

    #[must_use]
    pub fn original(&self) -> Option<&swiftide::chat_completion::ChatMessage> {
        self.original.as_ref()
    }

    #[must_use]
    pub fn maybe_completed_tool_call(&self) -> Option<&swiftide::chat_completion::ToolCall> {
        match self.original() {
            Some(swiftide::chat_completion::ChatMessage::ToolOutput(tool_call, ..)) => {
                Some(tool_call)
            }
            _ => None,
        }
    }
}

impl From<swiftide::chat_completion::ChatMessage> for ChatMessage {
    fn from(msg: swiftide::chat_completion::ChatMessage) -> Self {
        let mut builder = match &msg {
            swiftide::chat_completion::ChatMessage::System(msg) => ChatMessage::new_system(msg),
            swiftide::chat_completion::ChatMessage::User(msg) => ChatMessage::new_user(msg),
            swiftide::chat_completion::ChatMessage::Assistant(msg, ..) => {
                ChatMessage::new_assistant(msg.as_deref().unwrap_or_default())
            }
            swiftide::chat_completion::ChatMessage::ToolOutput(tool_call, _) => {
                ChatMessage::new_tool(format!("tool `{}` completed", tool_call.name()))
            }
            swiftide::chat_completion::ChatMessage::Summary(_) => unimplemented!(),
        };

        builder.with_original(msg).to_owned()
    }
}
