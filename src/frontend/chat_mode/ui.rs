use ratatui::prelude::*;
use ratatui::widgets::{Block, Padding, Paragraph};

use crate::frontend::App;

use super::{
    chat_list_widget::ChatListWidget, chat_messages_widget::ChatMessagesWidget,
    help_section_widget::HelpSectionWidget, input_bar_widget::InputBarWidget,
};

pub fn ui(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    // Create the main layout (vertical)
    let [main_area, bottom_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Main area
            Constraint::Length(2), // Commands display area
        ])
        .areas(area);

    // Split the main area into two columns
    let [chat_area, right_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80), // Left column (chat messages)
            Constraint::Percentage(20), // Right column (other info)
        ])
        .areas(main_area);

    let [chat_messages, input_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(8)])
        .spacing(0)
        .areas(chat_area);

    // Render chat messages
    ChatMessagesWidget::render(f, app, chat_messages);

    let [chat_list, help_area] =
        Layout::vertical([Constraint::Min(10), Constraint::Length(20)]).areas(right_area);

    // Render chat list
    ChatListWidget::render(f, app, chat_list);

    // Render user input bar
    InputBarWidget::render(f, app, input_area);

    // Render help section
    HelpSectionWidget::render(f, app, help_area);

    // Bottom paragraph with the uuid and git branch, right aligned italic
    let branch_name = if let Some(current_chat) = app.current_chat() {
        current_chat
            .branch_name
            .as_deref()
            .unwrap_or("not yet named")
    } else {
        "not yet named"
    };
    Paragraph::new(Line::from(vec![Span::raw(format!(
        "uuid: {}   branch-name: {}",
        app.current_chat_uuid, branch_name
    ))]))
    .style(Style::default().fg(Color::DarkGray).italic())
    .block(Block::default().padding(Padding::right(1)))
    .alignment(Alignment::Right)
    .render(bottom_area, f.buffer_mut());
}
