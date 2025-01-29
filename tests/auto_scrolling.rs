use kwaak::chat_message::ChatMessage;
use kwaak::commands::Command;
use kwaak::frontend::{ui, UIEvent, UserInputCommand};
use kwaak::test_utils::{setup_integration, IntegrationContext};
use kwaak::{assert_agent_responded, assert_command_done};

#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn auto_scrolling() {
    let mut context = setup_integration().await.unwrap();

    for i in 0..100 {
        let event = UIEvent::ChatMessage(context.uuid, ChatMessage::new_user(format!("hello {i}")));
        context.app.send_ui_event(event);

        context.render_ui();

        context
            .app
            .handle_events_until(UIEvent::is_scroll_end)
            .await;
    }

    // Current chat records the number of rendred lines previously, so we need to call it twice
    insta::assert_snapshot!("auto_scrolled", context.render_ui());
}
