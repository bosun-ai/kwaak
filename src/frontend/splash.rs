use layout::Flex;
use ratatui::{
    widgets::{Block, Padding},
    Frame,
};
use ratatui_splash_screen::{SplashConfig, SplashScreen};

// use std::error::Error;
// use std::io::stdout;
// use std::time::Duration;
//
use ratatui::prelude::*;
// use ratatui_splash_screen::{SplashConfig, SplashScreen, SplashError};
//
static SPLASH_CONFIG: SplashConfig = SplashConfig {
    image_data: include_bytes!("../../images/logo_term_nobg.png"),
    sha256sum: None,
    render_steps: 6,
    use_colors: true,
};

pub struct Splash {
    splash_screen: SplashScreen,
}

impl Default for Splash {
    fn default() -> Self {
        let splash_screen = SplashScreen::new(SPLASH_CONFIG).unwrap();
        Splash { splash_screen }
    }
}

// Splash renders in the center of the screen an image with a loading bar
// Image is 50x50 characters, below a line with the text "Indexing your code ..."
impl Splash {
    pub fn render(&mut self, f: &mut Frame, status_text: &str) {
        let splash_area = center(
            f.area(),
            Constraint::Percentage(48),
            Constraint::Percentage(67),
        );
        let [image_area, text_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(splash_area);

        f.render_widget(&mut self.splash_screen, image_area);

        #[allow(clippy::cast_possible_truncation)]
        let left_padding = (text_area.width - status_text.len() as u16) / 2;
        let block = Block::default().padding(Padding::new(left_padding, 0, 1, 0));
        let throbber = throbber_widgets_tui::Throbber::default().label(status_text);

        f.render_widget(throbber, block.inner(text_area));
        f.render_widget(block, text_area);
    }

    pub fn is_rendered(&self) -> bool {
        self.splash_screen.is_rendered()
    }
}

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}
