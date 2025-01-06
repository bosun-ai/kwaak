use insta::assert_snapshot;
use ratatui::{backend::TestBackend, Terminal};

use crate::frontend::{chat_mode, App};

/// Simple snapshots for now so we can refactor later.
#[test]
fn test_render_app() {
    let mut app = App::default();
    let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();

    terminal
        .draw(|f| chat_mode::ui(f, f.area(), &mut app))
        .unwrap();
    assert_snapshot!(terminal.backend());
}
