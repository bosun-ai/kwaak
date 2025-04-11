//! Handles all the actions that can be performed in the frontend based on `UIEvent`
//!
//! NOTE: if we can remove the dependency on app, this could be so much nicer
use std::collections::HashMap;

use copypasta::{ClipboardContext, ClipboardProvider as _};
use strum::{EnumMessage, IntoEnumIterator};

use crate::{chat::Chat, chat_message::ChatMessage, commands::Command, templates::Templates};

use super::{ui_input_command::UserInputCommand, App};

mod diff;
mod github;
mod scroll;

pub use diff::{diff_pull, diff_show};
pub use github::github_issue;
pub use scroll::{scroll_down, scroll_end, scroll_up};

pub fn delete_chat(app: &mut App) {
    let uuid = app.current_chat_uuid;
    app.dispatch_command(uuid, Command::StopAgent);
    // Remove the chat with the given UUID
    app.chats.retain(|chat| chat.uuid != uuid);

    if app.chats.is_empty() {
        app.add_chat(Chat::default());
        app.chats_state.select(Some(0));
        app.add_chat_message(
            app.current_chat_uuid,
            ChatMessage::new_system("Nice, you managed to delete the last chat!"),
        );
    } else {
        app.next_chat();
    }
}

pub fn copy_last_message(app: &mut App) {
    let Some(last_message) = app
        .current_chat()
        .and_then(|c| {
            c.messages
                .iter()
                .filter(|m| {
                    (m.role().is_assistant() || m.role().is_user()) && !m.content().is_empty()
                })
                .next_back()
        })
        .map(ChatMessage::content)
    else {
        app.add_chat_message(
            app.current_chat_uuid,
            ChatMessage::new_system("No message to copy"),
        );
        return;
    }; // Replace with actual retrieval of the last message
       //
    if let Err(e) =
        ClipboardContext::new().and_then(|mut ctx| ctx.set_contents(last_message.to_string()))
    {
        tracing::error!("Error copying last message to clipboard {e:#}");
        return;
    }
    app.add_chat_message(
        app.current_chat_uuid,
        ChatMessage::new_system("Copied last message to clipboard"),
    );
}

pub fn help(app: &mut App) {
    let message = help_message();

    app.add_chat_message(app.current_chat_uuid, ChatMessage::new_system(message));
}

fn help_message() -> String {
    let input_commands_with_description = UserInputCommand::iter()
        .map(|i| (i.to_string(), i.get_documentation()))
        .collect::<HashMap<_, _>>();

    let mut context = tera::Context::new();
    context.insert("commands", &input_commands_with_description);

    Templates::render("help.md", &context).expect("failed to render help message; nice")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_message() {
        let message = help_message();
        insta::assert_snapshot!(message);
    }
}
