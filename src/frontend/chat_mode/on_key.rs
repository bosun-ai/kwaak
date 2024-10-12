use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    chat_message::{ChatMessage, ChatMessageBuilder},
    commands::Command,
    frontend::{App, UIEvent, UserInputCommand},
};

pub fn on_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Tab => app.send_ui_event(UIEvent::NextChat),
        KeyCode::Down => {
            app.vertical_scroll = app.vertical_scroll.saturating_add(1);
            app.vertical_scroll_state = app
                .vertical_scroll_state
                .position(app.vertical_scroll as usize);
        }
        KeyCode::Up => {
            app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
            app.vertical_scroll_state = app
                .vertical_scroll_state
                .position(app.vertical_scroll as usize);
        }
        KeyCode::Char(c) => {
            app.input.push(c);
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Enter if !app.input.is_empty() => {
            let message = if app.input.starts_with('/') {
                handle_input_command(app)
            } else {
                // Currently just dispatch a user message command and answer the query
                // Later, perhaps maint a 'chat', add message to that chat, and then send
                // the whole thing
                app.dispatch_command(&Command::Chat {
                    message: app.input.clone(),
                    uuid: app.current_chat,
                });

                ChatMessage::new_user(&app.input)
                    .uuid(app.current_chat)
                    .to_owned()
            };

            app.send_ui_event(message);

            app.input.clear();
        }
        _ => {}
    }
}

pub fn handle_input_command(app: &App) -> ChatMessageBuilder {
    let Ok(cmd) = app.input[1..].parse::<UserInputCommand>() else {
        return ChatMessage::new_system("Unknown command")
            .uuid(app.current_chat)
            .to_owned();
    };

    if let Some(cmd) = cmd.to_command(app.current_chat) {
        // If the backend supports it, forward the command
        app.dispatch_command(&cmd);
    } else if let Ok(cmd) = UIEvent::try_from(cmd) {
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
