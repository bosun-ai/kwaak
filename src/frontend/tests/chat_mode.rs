use insta::assert_snapshot;
use ratatui::{backend::TestBackend, Terminal};
use uuid::Uuid;

use crate::frontend::{chat_mode, App};

/// Simple snapshots for now so we can refactor later.
#[tokio::test]
async fn test_render_app() {
    let mut app = App::default();
    let fixed_uuid = Uuid::parse_str("936DA01F9ABD4d9d80C702AF85C822A8").unwrap();
    app.current_chat_uuid = fixed_uuid;
    let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();

    terminal
        .draw(|f| chat_mode::ui(f, f.area(), &mut app))
        .unwrap();
    assert_snapshot!(terminal.backend());
}
