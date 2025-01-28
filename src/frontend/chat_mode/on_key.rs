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

    let mut update_auto_tail = None;

    if let Some(current_chat) = app.current_chat_mut() {
        match key.code {
            KeyCode::End => {
                update_auto_tail = Some(true);
            }
            KeyCode::PageDown | KeyCode::Down => {
                // Disable auto-tail if user scrolls down manually, but keep enabled if at end
                if current_chat.vertical_scroll < current_chat.num_lines.saturating_sub(1) {
                    update_auto_tail = Some(false);
                }
            }
            KeyCode::PageUp | KeyCode::Up => {
                update_auto_tail = Some(false);
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::End => app.send_ui_event(UIEvent::ScrollEnd),
        KeyCode::PageDown | KeyCode::Down => app.send_ui_event(UIEvent::ScrollDown),
        KeyCode::PageUp | KeyCode::Up => app.send_ui_event(UIEvent::ScrollUp),
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

    if let (Some(current_chat), Some(auto_tail)) = (app.current_chat_mut(), update_auto_tail) {
        current_chat.auto_tail = auto_tail;
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
