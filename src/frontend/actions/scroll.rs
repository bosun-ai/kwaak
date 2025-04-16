use crate::frontend::App;

pub fn scroll_up(app: &mut App) {
    let current_chat = app.current_chat_mut();
    current_chat.vertical_scroll = current_chat.vertical_scroll.saturating_sub(2);
    current_chat.vertical_scroll_state = current_chat
        .vertical_scroll_state
        .position(current_chat.vertical_scroll);
    current_chat.auto_tail = false;
}

pub fn scroll_down(app: &mut App) {
    let current_chat = app.current_chat_mut();
    current_chat.vertical_scroll = current_chat.vertical_scroll.saturating_add(2);
    current_chat.vertical_scroll_state = current_chat
        .vertical_scroll_state
        .position(current_chat.vertical_scroll);
    // Optional: only disable auto_tail when actually scrolling up
    current_chat.auto_tail = false;
}

pub fn scroll_end(app: &mut App) {
    let max_lines_in_area = app.chat_messages_max_lines.saturating_sub(2);

    let current_chat = app.current_chat_mut();
    let scroll_position = current_chat
        .num_lines
        .saturating_sub(max_lines_in_area as usize);

    current_chat.vertical_scroll = scroll_position;
    current_chat.vertical_scroll_state =
        current_chat.vertical_scroll_state.position(scroll_position);
    current_chat.auto_tail = true;
}
