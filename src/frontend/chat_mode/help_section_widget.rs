use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

use crate::frontend::{App, UserInputCommand};

pub struct HelpSectionWidget;

impl HelpSectionWidget {
    pub fn render(f: &mut ratatui::Frame, app: &App, area: Rect) {
        let border_set = symbols::border::Set {
            top_right: symbols::line::NORMAL.vertical_left,
            ..symbols::border::PLAIN
        };
        let [top, bottom] = Layout::vertical([
            #[allow(clippy::cast_possible_truncation)]
            Constraint::Length((app.supported_commands().len() / 2) as u16 + 3),
            Constraint::Min(9),
        ])
        .areas(area);

        let mut command_columns =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(top)
                .to_vec();

        let mut supported_commands = app.supported_commands();
        supported_commands.retain(|c| !c.is_help());
        supported_commands.insert(0, UserInputCommand::Help);

        // Only show the chat commands help block if the screen is big enough
        if top.height as usize > app.supported_commands().len() / 2 {
            Block::default()
                .title("Chat commands".bold())
                .title_alignment(Alignment::Center)
                .borders(Borders::TOP | Borders::RIGHT)
                .border_set(border_set)
                .padding(Padding::uniform(2))
                .render(top, f.buffer_mut());
        } else {
            Block::default()
                .borders(Borders::RIGHT)
                .border_set(border_set)
                .render(top, f.buffer_mut());
        }

        let (left_commands, right_commands) =
            supported_commands.split_at(supported_commands.len() / 2);

        for commands in &[left_commands, right_commands] {
            Paragraph::new(
                commands
                    .iter()
                    .map(|c| Line::from(format!("/{c}").bold()))
                    .collect::<Vec<Line>>(),
            )
            .block(Block::default().padding(Padding::uniform(2)))
            .render(command_columns.pop().unwrap(), f.buffer_mut());
        }
        Paragraph::new(
            [
                "Page Up/Down - Scroll",
                "End - Scroll to end",
                "Tab - Next chat",
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
