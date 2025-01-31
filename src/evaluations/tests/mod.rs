use super::logging_responder::LoggingResponder;
use crate::commands::{CommandResponse, Responder};
use swiftide::chat_completion::ChatMessage;
use uuid::Uuid;

#[test]
fn test_logging_responder_formatting() {
    let responder = LoggingResponder::new();

    // Test agent message with escaped characters
    let chat_message = ChatMessage::Assistant(
        Some(
            r#"Here's a message with "quotes" and \n newlines and a JSON: {"key": "value"}"#
                .to_string(),
        ),
        None,
    );
    responder.agent_message(chat_message);

    // Test command response with escaped characters
    let uuid = Uuid::new_v4();
    let command_response = CommandResponse::BackendMessage(
        uuid,
        r#"Message with "quotes" and \n newlines"#.to_string(),
    );
    responder.send(command_response);

    // Test system message with escaped characters
    responder.system_message(r#"System message with "quotes" and \n newlines"#);

    // Test update with escaped characters
    responder.update(r#"Update with "quotes" and \n newlines"#);

    // Get the log and verify formatting
    let log = responder.get_log();
    println!("Log output:\n{}", log);

    // The log should contain actual newlines and unescaped quotes
    assert!(log.contains(r#"Assistant(Some("Here's a message with "quotes" and "#));
    assert!(log.contains(r#"newlines and a JSON: {"key": "value"}")"#));
    assert!(log.contains(r#"BackendMessage("#));
    assert!(log.contains(r#", "Message with "quotes" and "#));
    assert!(log.contains("newlines"));
    assert!(log.contains(r#"System message with "quotes" and "#));
    assert!(log.contains("newlines"));
}
