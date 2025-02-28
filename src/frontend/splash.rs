use std::borrow::Cow;

use layout::Flex;
use ratatui::{
    Frame,
    widgets::{Block, Padding},
};
use ratatui_splash_screen::{SplashConfig, SplashScreen};

// use std::error::Error;
// use std::io::stdout;
// use std::time::Duration;
//
use ratatui::prelude::*;
use text::ToSpan as _;
// use ratatui_splash_screen::{SplashConfig, SplashScreen, SplashError};
//
static SPLASH_CONFIG: SplashConfig = SplashConfig {
    image_data: include_bytes!("../../images/logo_term_nobg.png"),
    sha256sum: None,
    render_steps: 6,
    use_colors: true,
};

pub struct Splash<'m> {
    splash_screen: SplashScreen,
    message: Cow<'m, str>, // Might get tens of thousands of updates (or more), so let's avoid unnecessary allocations
}

impl Default for Splash<'_> {
    fn default() -> Self {
        let splash_screen = SplashScreen::new(SPLASH_CONFIG).unwrap();
        Splash {
            splash_screen,
            message: "".into(),
        }
    }
}

// Splash renders in the center of the screen an image with a loading bar
// Image is 50x50 characters, below a line with the text "Indexing your code ..."
impl<'m> Splash<'m> {
    pub fn render(&mut self, f: &mut Frame) {
        let splash_area = center(
            f.area(),
            Constraint::Percentage(48),
            Constraint::Percentage(67),
        );
        let [image_area, text_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(splash_area);

        f.render_widget(&mut self.splash_screen, image_area);

        #[allow(clippy::cast_possible_truncation)]
        let left_padding = (text_area.width - self.message.len() as u16) / 2;
        let block = Block::default().padding(Padding::new(left_padding, 0, 1, 0));
        let throbber = throbber_widgets_tui::Throbber::default().label(self.message.to_span());

        f.render_widget(throbber, block.inner(text_area));
        f.render_widget(block, text_area);
    }

    pub fn is_rendered(&self) -> bool {
        self.splash_screen.is_rendered()
    }

    pub fn set_message<T>(&mut self, message: T)
    where
        T: Into<Cow<'m, str>>,
    {
        self.message = message.into();
    }
}

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}
