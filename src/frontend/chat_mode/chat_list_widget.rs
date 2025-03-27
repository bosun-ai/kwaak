use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, HighlightSpacing, List, ListItem, Padding};

use crate::chat::Chat;
use crate::frontend::App;

pub struct ChatListWidget;

impl ChatListWidget {
    pub fn render(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
        let list: List = app
            .chats
            .iter()
            .map(format_chat_in_list)
            .collect::<List>()
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
            .block(
                Block::default()
                    .title("Chats".bold())
                    .title_alignment(Alignment::Center)
                    .borders(Borders::TOP | Borders::RIGHT)
                    .padding(Padding::horizontal(1)),
            );

        f.render_stateful_widget(list, area, &mut app.chats_state);
    }
}

fn format_chat_in_list<S>(chat: &Chat<S>) -> ListItem {
    const LOADING: &str = "";
    const CAN_MESSAGE: &str = "󰍩";
    const NEW_MESSAGE: &str = "󱥁";
    const MESSAGE_LOCK: &str = "󱅳";

    let prefix = if chat.is_loading() && chat.new_message_count > 0 {
        MESSAGE_LOCK
    } else if chat.is_loading() {
        LOADING
    } else if chat.new_message_count > 0 {
        NEW_MESSAGE
    } else {
        CAN_MESSAGE
    };

    ListItem::from(format!("{prefix}  {name}", name = chat.name))
}
