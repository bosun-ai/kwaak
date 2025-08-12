use {
    crate::{
        chat_message::ChatMessage, commands::Command, frontend::App, frontend::ui_event::UIEvent,
    },
    uuid::Uuid,
};

pub async fn github_issue(app: &mut App<'_>, number: u64, uuid: Uuid) {
    let Some(chat) = app.find_chat(uuid) else {
        app.add_chat_message(uuid, ChatMessage::new_system("No chat found in uuid"));
        return;
    };

    let Ok(Some(github_session)) = chat.repository.github_session().await else {
        app.add_chat_message(
                uuid,
                ChatMessage::new_system("GitHub is not available for this repository; if that is not intended check the logs for errors".to_string()),
            );
        return;
    };

    let issue_with_comments = match github_session.fetch_issue(number).await {
        Ok(issue) => issue,
        Err(e) => {
            app.add_chat_message(
                uuid,
                ChatMessage::new_system(format!("Failed to fetch GitHub issue #{number}: {e}",)),
            );
            return;
        }
    };

    let issue_md = issue_with_comments.markdown();
    let prompt = format!(
        "Please summarize, analyze, and then proceed to fix the following issue. Take \
                    into account suggested fixes proposed in the issue description and comments. \
                    \n\n{issue_md}"
    );
    app.dispatch_command(
        uuid,
        Command::Chat {
            message: prompt.clone(),
        },
    );
    let message = ChatMessage::new_user(prompt);
    app.add_chat_message(uuid, message);
    if let Some(chat) = app.find_chat_mut(uuid)
        && chat.auto_tail
    {
        app.send_ui_event(UIEvent::ScrollEnd);
    }
}
