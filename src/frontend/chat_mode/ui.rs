use ratatui::prelude::*;
use ratatui::widgets::{
    Clear, HighlightSpacing, List, Padding, Scrollbar, ScrollbarOrientation, Wrap,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};

use crate::chat_message::{ChatMessage, ChatRole};
use crate::frontend::App;

pub fn ui(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    // Create the main layout (vertical)
    let [main_area, input_area, help_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Main area
            Constraint::Length(3), // User input bar (2 lines)
            Constraint::Length(3), // Commands display area
        ])
        .areas(area);

    // Split the main area into two columns
    let [chat_messages, chat_list] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80), // Left column (chat messages)
            Constraint::Percentage(20), // Right column (other info)
        ])
        .areas(main_area);

    // Render chat messages
    render_chat_messages(f, app, chat_messages);

    // Render other information
    render_chat_list(f, app, chat_list);

    // Render user input bar
    render_input_bar(f, app, input_area);

    // Render commands display area
    render_commands_display(f, app, help_area);
}

fn render_chat_messages(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let messages = app.current_chat().messages.clone();
    let chat_content: Text = messages.iter().flat_map(format_chat_message).collect();

    let num_lines = chat_content.lines.len();

    app.vertical_scroll_state = app.vertical_scroll_state.content_length(num_lines);

    let chat_messages = Paragraph::new(chat_content)
        .block(
            Block::default()
                .title("Chat")
                .borders(Borders::ALL)
                .padding(Padding::horizontal(1)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.vertical_scroll, 0));

    f.render_widget(chat_messages, area);

    // Render scrollbar
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        area,
        &mut app.vertical_scroll_state,
    );
}
fn render_chat_list(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let list: List = app
        .chats
        .iter()
        .map(|chat| chat.name.as_str())
        .collect::<List>()
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
        .block(Block::default().title("Chats").borders(Borders::ALL));

    f.render_stateful_widget(list, area, &mut app.chats_state);
}

fn render_input_bar(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let input = Paragraph::new(app.input.as_str())
        .block(Block::default().title("Input").borders(Borders::ALL));
    f.render_widget(input, area);
    // Set cursor position
    f.set_cursor_position(
        // Put cursor past the end of the input text
        #[allow(clippy::cast_possible_truncation)]
        (area.x + app.input.len() as u16 + 1, area.y + 1),
    );
}

fn render_commands_display(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let commands = Paragraph::new(
        app.supported_commands()
            .iter()
            .map(|c| format!("/{c}"))
            .collect::<Vec<_>>()
            .join(" "),
    )
    .wrap(Wrap { trim: true })
    .block(Block::default().title("Commands").borders(Borders::ALL));
    f.render_widget(commands, area);
}

fn format_chat_message(message: &ChatMessage) -> Text {
    let prefix: Span = Span::styled(message.role().as_ref(), Style::default().fg(Color::Yellow));
    let content: Text = tui_markdown::from_str(message.content());

    Text::from(prefix) + content
}
