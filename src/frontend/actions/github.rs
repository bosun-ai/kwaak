use {
    crate::{
        chat_message::ChatMessage, commands::Command, frontend::ui_event::UIEvent, frontend::App,
        git,
    },
    uuid::Uuid,
};

pub async fn github_issue(app: &mut App<'_>, number: u64, uuid: Uuid) {
    let Some(ref repository) = app.repository else {
        app.add_chat_message(uuid, ChatMessage::new_system("No repository found in UI"));
        return;
    };
    let github_session = match git::github::GithubSession::from_repository(repository) {
        Ok(session) => session,
        Err(e) => {
            app.add_chat_message(
                uuid,
                ChatMessage::new_system(format!("Failed to create GitHub session: {e}")),
            );
            return;
        }
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

    let issue_md = github_session.issue_to_markdown(&issue_with_comments);
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
    if let Some(chat) = app.find_chat_mut(uuid) {
        if chat.auto_tail {
            app.send_ui_event(UIEvent::ScrollEnd);
        }
    }
}
