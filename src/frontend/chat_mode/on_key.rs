use crossterm::event::{KeyCode, KeyEvent};
use tui_textarea::TextArea;

use crate::{
    chat_message::{ChatMessage, ChatMessageBuilder},
    commands::Command,
    frontend::{App, UIEvent, UserInputCommand},
};

pub fn on_key(app: &mut App, key: KeyEvent) {
    let current_input = app.text_input.lines().join("\n");

    // `Ctrl-s` to send the message in the text input
    if key.code == KeyCode::Char('s')
        && key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
        && !current_input.is_empty()
    {
        let message = if current_input.starts_with('/') {
            handle_input_command(app)
        } else {
            // Currently just dispatch a user message command and answer the query
            // Later, perhaps maint a 'chat', add message to that chat, and then send
            // the whole thing
            app.dispatch_command(&Command::Chat {
                message: current_input.clone(),
                uuid: app.current_chat,
            });

            ChatMessage::new_user(&current_input)
                .uuid(app.current_chat)
                .to_owned()
        };

        app.send_ui_event(message);

        app.text_input = TextArea::default();

        return;
    }

    // `Ctrl-x` to stop a running agent
    if key.code == KeyCode::Char('x')
        && key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        app.dispatch_command(&Command::StopAgent {
            uuid: app.current_chat,
        });
        return;
    }

    // `Ctrl-n` to start a new chat
    if key.code == KeyCode::Char('n')
        && key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        app.send_ui_event(UIEvent::NewChat);
        return;
    }

    match key.code {
        KeyCode::Tab => app.send_ui_event(UIEvent::NextChat),
        KeyCode::PageDown => {
            app.vertical_scroll = app.vertical_scroll.saturating_add(1);
            app.vertical_scroll_state = app.vertical_scroll_state.position(app.vertical_scroll);
        }
        KeyCode::PageUp => {
            app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
            app.vertical_scroll_state = app.vertical_scroll_state.position(app.vertical_scroll);
        }
        _ => {
            app.text_input.input(key);
        }
    }
}

pub fn handle_input_command(app: &mut App) -> ChatMessageBuilder {
    let current_input = app.text_input.lines().join("\n");

    let Ok(cmd) = UserInputCommand::parse_from_input(&current_input) else {
        return ChatMessage::new_system("Unknown command")
            .uuid(app.current_chat)
            .to_owned();
    };

    if let Some(cmd) = cmd.to_command(app.current_chat) {
        // If the backend supports it, forward the command
        app.dispatch_command(&cmd);
    } else if let Ok(cmd) = UIEvent::try_from(cmd.clone()) {
        app.send_ui_event(cmd);
    } else {
        tracing::error!("Could not convert ui command to backend command nor ui event {cmd}");
        return ChatMessage::new_system("Unknown command")
            .uuid(app.current_chat)
            .to_owned();
    }

    ChatMessage::new_command(cmd.as_ref())
        .uuid(app.current_chat)
        .to_owned()

    // Display the command as a message
}
