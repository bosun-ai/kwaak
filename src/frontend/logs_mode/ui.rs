use log::LevelFilter;
use ratatui::{prelude::*, widgets::*};
use tui_logger::{TuiLoggerLevelOutput, TuiLoggerSmartWidget, TuiLoggerWidget, TuiWidgetState};

use crate::frontend::App;

pub fn ui(f: &mut ratatui::Frame, area: Rect, app: &mut App) {
    let buf = f.buffer_mut();
    let [smart_area, help_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(area);

    TuiLoggerSmartWidget::default()
        .style_error(Style::default().fg(Color::Red))
        .style_debug(Style::default().fg(Color::Green))
        .style_warn(Style::default().fg(Color::Yellow))
        .style_trace(Style::default().fg(Color::Magenta))
        .style_info(Style::default().fg(Color::Cyan))
        .output_separator(':')
        .output_timestamp(Some("%H:%M:%S".to_string()))
        .output_level(Some(TuiLoggerLevelOutput::Abbreviated))
        .output_target(true)
        .output_file(true)
        .output_line(true)
        .title_log("Logs")
        .title_target("Filters")
        .state(&app.log_state)
        .render(smart_area, buf);

    if area.width > 40 {
        Text::from(vec![
            "Q: Quit | Tab: Switch state | ↑/↓: Select target | f: Focus target".into(),
            "←/→: Display level | +/-: Filter level | Space: Toggle hidden targets".into(),
            "h: Hide target selector | PageUp/Down: Scroll | Esc: Cancel scroll".into(),
        ])
        .style(Color::Gray)
        .centered()
        .render(help_area, buf);
    }
}
