use ratatui::prelude::*;
use ratatui::widgets::{
    HighlightSpacing, List, ListItem, Padding, Scrollbar, ScrollbarOrientation, Wrap,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use crate::chat::{Chat, ChatState};
use crate::frontend::App;

use super::message_formatting::format_chat_message;

pub fn ui(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    // Create the main layout (vertical)
    let [main_area, bottom_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0), // Main area
            // Constraint::Length(5), // User input bar (2 lines)
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
    render_chat_messages(f, app, chat_messages);

    let [chat_list, help_area] =
        Layout::vertical([Constraint::Min(10), Constraint::Length(20)]).areas(right_area);
    // Render other information
    render_chat_list(f, app, chat_list);

    // Render user input bar
    render_input_bar(f, app, input_area);

    // Render commands display area
    render_help(f, app, help_area);

    // Bottom paragraph with the git branch, right aligned italic

    Paragraph::new(Line::from(vec![Span::raw(format!(
        "kwaak/{}",
        app.current_chat
    ))]))
    .style(Style::default().fg(Color::DarkGray).italic())
    .block(Block::default().padding(Padding::right(1)))
    .alignment(Alignment::Right)
    .render(bottom_area, f.buffer_mut());
}

fn render_chat_messages(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let Some(current_chat) = app.current_chat_mut() else {
        return;
    };
    let messages = current_chat.messages.clone();
    let chat_content: Text = messages
        .iter()
        .flat_map(|m| format_chat_message(current_chat, m))
        .collect();

    // Since we are rendering the chat, we can reset the new message count
    current_chat.new_message_count = 0;

    // We need to consider the available area height to calculate how much can be shown
    current_chat.num_lines = chat_content.lines.len();

    // Record the number of lines in the chat for multi line scrolling
    current_chat.vertical_scroll_state = current_chat
        .vertical_scroll_state
        .content_length(current_chat.num_lines);

    // Max scroll to halfway view-height of last content
    if current_chat.vertical_scroll >= current_chat.num_lines.saturating_sub(1) {
        current_chat.vertical_scroll = current_chat.num_lines.saturating_sub(1);
    }

    // Unify borders
    let border_set = symbols::border::Set {
        top_right: symbols::line::NORMAL.horizontal_down,
        ..symbols::border::PLAIN
    };

    let message_block = Block::default()
        .border_set(border_set)
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .padding(Padding::horizontal(1));

    #[allow(clippy::cast_possible_truncation)]
    let chat_messages = Paragraph::new(chat_content)
        .block(message_block)
        .wrap(Wrap { trim: false })
        .scroll((current_chat.vertical_scroll as u16, 0));

    f.render_widget(chat_messages, area);

    // Render scrollbar
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑")) // Fixed the unterminated string
            .end_symbol(Some("↓")),
        area,
        &mut current_chat.vertical_scroll_state,
    );
}

fn render_chat_list(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
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

fn format_chat_in_list(chat: &Chat) -> ListItem {
    const ELLIPSIS: &str = "…";
    const CAN_MESSAGE: &str = "🍩";
    const NEW_MESSAGE: &str = "📩";
    const MESSAGE_LOCK: &str = "🔒";

    let prefix = if chat.is_loading() && chat.new_message_count > 0 {
        MESSAGE_LOCK
    } else if chat.is_loading() {
        ELLIPSIS
    } else if chat.new_message_count > 0 {
        NEW_MESSAGE
    } else {
        CAN_MESSAGE
    };

    ListItem::from(format!("{prefix}  {name}", name = chat.name))
}

fn render_input_bar(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let border_set = symbols::border::Set {
        top_left: symbols::line::NORMAL.vertical_right,
        top_right: symbols::line::NORMAL.vertical_left,
        bottom_right: symbols::line::NORMAL.horizontal_up,
        ..symbols::border::PLAIN
    };

    let block = Block::default()
        .border_set(border_set)
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL);

    if app.current_chat().is_some_and(Chat::is_loading) {
        let loading_msg = match &app.current_chat().expect("infallible").state {
            ChatState::Loading => "Kwaaking ...".to_string(),
            ChatState::LoadingWithMessage(msg) => format!("Kwaaking ({msg}) ..."),
            ChatState::Ready => unreachable!(),
        };
        let throbber = throbber_widgets_tui::Throbber::default().label(&loading_msg);

        f.render_widget(throbber, block.inner(area));
        return block.render(area, f.buffer_mut());
    }

    // let input = Paragraph::new(app.input.as_str()).block(block);
    app.text_input.set_block(block);
    f.render_widget(&app.text_input, area);
    // Set cursor position
    // f.set_cursor_position(
    //     // Put cursor past the end of the input text
    //     #[allow(clippy::cast_possible_truncation)]
    //     (area.x + app.input.len() as u16 + 1, area.y + 1),
    // );
}

fn render_help(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let border_set = symbols::border::Set {
        top_right: symbols::line::NORMAL.vertical_left,
        ..symbols::border::PLAIN
    };
    let [top, bottom] = Layout::vertical([
        #[allow(clippy::cast_possible_truncation)]
        Constraint::Length(app.supported_commands().len() as u16 + 3),
        Constraint::Min(4),
    ])
    .areas(area);

    Paragraph::new(
        app.supported_commands()
            .iter()
            .map(|c| Line::from(format!("/{c}").bold()))
            .collect::<Vec<Line>>(),
    )
    .block(
        Block::default()
            .title("Chat commands".bold())
            .title_alignment(Alignment::Center)
            .borders(Borders::TOP | Borders::RIGHT)
            .border_set(border_set)
            .padding(Padding::uniform(1)),
    )
    .render(top, f.buffer_mut());

    let border_set = symbols::border::Set {
        top_right: symbols::line::NORMAL.vertical_left,
        ..symbols::border::PLAIN
    };
    Paragraph::new(
        [
            "Page Up/Down - Scroll",
            "End - Scroll to end",
            "^s - Send message",
            "^x - Stop agent",
            "^n - New chat",
            "^q - Quit", // Updated the keybinding here
        ]
        .iter()
        .map(|h| Line::from(h.bold()))
        .collect::<Vec<Line>>(),
    )
    .block(
        Block::default()
            .title("Keybindings".bold())
            .title_alignment(Alignment::Center)
            .border_set(border_set)
            .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
            .padding(Padding::uniform(1)),
    )
    .render(bottom, f.buffer_mut());
}
