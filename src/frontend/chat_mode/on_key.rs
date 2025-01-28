use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    chat_message::ChatMessage,
    commands::Command,
    frontend::{ui_event::UIEvent, ui_input_command::UserInputCommand, App},
};

pub fn on_key(app: &mut App, key: &KeyEvent) {
    let current_input = app.text_input.lines().join("\n");

    // `Ctrl-Enter` or `Shift-Enter` or `Ctrl-s` to send the message in the text input
    if (key.code == KeyCode::Char('s')
        && key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL))
        || (key.code == KeyCode::Enter
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL))
        || (key.code == KeyCode::Enter
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::SHIFT))
            && !current_input.is_empty()
    {
        let message = if current_input.starts_with('/') {
            handle_input_command(app)
        } else {
            app.dispatch_command(
                app.current_chat_uuid,
                Command::Chat {
                    message: current_input.clone(),
                },
            );

            ChatMessage::new_user(current_input)
        };

        app.send_ui_event(UIEvent::ChatMessage(app.current_chat_uuid, message));

        app.reset_text_input();

        return;
    }

    // `Ctrl-x` to stop a running agent
    if key.code == KeyCode::Char('x')
        && key
            .modifiers
            .contains(crossterm::event::KeyModifiers::CONTROL)
    {
        app.dispatch_command(app.current_chat_uuid, Command::StopAgent);
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

    let auto_tail_update = if let Some(current_chat) = app.current_chat_mut() {
        match key.code {
            KeyCode::End => {
                app.send_ui_event(UIEvent::ScrollEnd);
                Some(true)
            }
            KeyCode::PageDown | KeyCode::Down => {
                app.send_ui_event(UIEvent::ScrollDown);
                if current_chat.vertical_scroll < current_chat.num_lines.saturating_sub(1) {
                    Some(false)
                } else {
                    None
                }
            }
            KeyCode::PageUp | KeyCode::Up => {
                app.send_ui_event(UIEvent::ScrollUp);
                Some(false)
            }
            _ => None,
        }
    } else {
        None
    };

    if let Some(current_chat) = app.current_chat_mut() {
        if let Some(auto_tail) = auto_tail_update {
            current_chat.auto_tail = auto_tail;
        }
    }

    match key.code {
        KeyCode::Tab => app.send_ui_event(UIEvent::NextChat),
        _ => {
            // Hack to get linewrapping to work with tui_textarea
            if let Some(last_line) = app.text_input.lines().last() {
                if let Some(input_width) = app.input_width {
                    if last_line.len() >= input_width as usize && key.code == KeyCode::Char(' ') {
                        app.text_input.insert_newline();
                        return;
                    }
                }
            }
            app.text_input.input(*key);
        }
    }
}

pub fn handle_input_command(app: &mut App) -> ChatMessage {
    let current_input = app.text_input.lines().join("\n");

    let Ok(cmd) = UserInputCommand::parse_from_input(&current_input) else {
        return ChatMessage::new_system("Unknown command").clone();
    };

    let message = ChatMessage::new_command(cmd.as_ref()).clone();

    app.send_ui_event(UIEvent::UserInputCommand(app.current_chat_uuid, cmd));

    message
}
