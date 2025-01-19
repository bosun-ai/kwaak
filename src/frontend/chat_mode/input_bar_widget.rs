use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use crate::chat::Chat;
use crate::frontend::App;

pub struct InputBarWidget;

impl InputBarWidget {
    pub fn render(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
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
            block.render(area, f.buffer_mut());
        } else {
            app.text_input.set_block(block);
            f.render_widget(&app.text_input, area);
        }
    }
}
