use derive_builder::Builder;
use uuid::Uuid;

/// Represents a chat message that can be stored in a [`Chat`]
///
/// Messages are expected to be formatted strings and are displayed as-is. Markdown is rendered
/// using `tui-markdown`.
#[derive(Clone, Default, Builder, PartialEq)]
#[builder(setter(into, strip_option), build_fn(skip))]
pub struct ChatMessage {
    role: ChatRole,
    content: String,
    original: Option<swiftide::chat_completion::ChatMessage>,
    uuid: Option<Uuid>,
}

// Debug with truncated content
impl std::fmt::Debug for ChatMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatMessage")
            .field("role", &self.role)
            .field(
                "content",
                &self.content[..std::cmp::min(10, self.content.len())].to_string(),
            )
            .field("uuid", &self.uuid)
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
    pub fn new_user(msg: impl Into<String>) -> ChatMessageBuilder {
        ChatMessageBuilder::default()
            .role(ChatRole::User)
            .content(msg.into())
            .to_owned()
    }

    pub fn new_system(msg: impl Into<String>) -> ChatMessageBuilder {
        ChatMessageBuilder::default()
            .role(ChatRole::System)
            .content(msg.into())
            .to_owned()
    }

    pub fn new_command(cmd: impl Into<String>) -> ChatMessageBuilder {
        ChatMessageBuilder::default()
            .role(ChatRole::Command)
            .content(cmd.into().to_string())
            .to_owned()
    }

    pub fn new_assistant(msg: impl Into<String>) -> ChatMessageBuilder {
        ChatMessageBuilder::default()
            .role(ChatRole::Assistant)
            .content(msg.into())
            .to_owned()
    }

    pub fn new_tool(msg: impl Into<String>) -> ChatMessageBuilder {
        ChatMessageBuilder::default()
            .role(ChatRole::Tool)
            .content(msg.into())
            .to_owned()
    }

    #[must_use]
    pub fn uuid(&self) -> Option<Uuid> {
        self.uuid
    }
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
    #[must_use]
    pub fn role(&self) -> &ChatRole {
        &self.role
    }

    #[must_use]
    pub fn with_uuid(self, uuid: Uuid) -> Self {
        Self {
            uuid: Some(uuid),
            ..self
        }
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

        builder.original(msg).build()
    }
}

impl ChatMessageBuilder {
    // Building is infallible
    pub fn build(&mut self) -> ChatMessage {
        ChatMessage {
            content: self.content.clone().unwrap_or_default(),
            uuid: self.uuid.unwrap_or_default(),
            role: self.role.unwrap_or_default(),
            original: self.original.clone().unwrap_or_default(),
        }
    }
}
