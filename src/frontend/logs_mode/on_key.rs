use crossterm::event::{KeyCode, KeyEvent};
use tui_logger::TuiWidgetEvent;

use crate::frontend::{app::AppMode, App};

pub fn on_key(app: &mut App, key: KeyEvent) {
    let state = &mut app.log_state;

    match key.code {
        KeyCode::Char('q') => app.mode = AppMode::Quit,
        KeyCode::Char(' ') => state.transition(TuiWidgetEvent::SpaceKey),
        KeyCode::Esc => state.transition(TuiWidgetEvent::EscapeKey),
        KeyCode::PageUp => state.transition(TuiWidgetEvent::PrevPageKey),
        KeyCode::PageDown => state.transition(TuiWidgetEvent::NextPageKey),
        KeyCode::Up => state.transition(TuiWidgetEvent::UpKey),
        KeyCode::Down => state.transition(TuiWidgetEvent::DownKey),
        KeyCode::Left => state.transition(TuiWidgetEvent::LeftKey),
        KeyCode::Right => state.transition(TuiWidgetEvent::RightKey),
        KeyCode::Char('+') => state.transition(TuiWidgetEvent::PlusKey),
        KeyCode::Char('-') => state.transition(TuiWidgetEvent::MinusKey),
        KeyCode::Char('h') => state.transition(TuiWidgetEvent::HideKey),
        KeyCode::Char('f') => state.transition(TuiWidgetEvent::FocusKey),
        _ => (),
    }
}
