use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::frontend::App;

pub struct HelpSectionWidget;

impl HelpSectionWidget {
    pub fn render(f: &mut ratatui::Frame, app: &App, area: Rect) {
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

        Paragraph::new(
            [
                "Page Up/Down - Scroll",
                "End - Scroll to end",
                "^s - Send message",
                "^x - Stop agent",
                "^n - New chat",
                "^q - Quit",
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
}
