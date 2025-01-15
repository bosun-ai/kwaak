//! Handles all the actions that can be performed in the frontend based on `UIEvents`
//!
//! NOTE: if we can remove the dependency on app, this could be so much nicer
use copypasta::{ClipboardContext, ClipboardProvider as _};

use crate::{chat::Chat, chat_message::ChatMessage, commands::Command};

use super::App;

mod diff;

pub use diff::{diff_pull, diff_show};

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
                .filter(|m| m.role().is_assistant() || m.role().is_user())
                .last()
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
