use std::sync::Arc;

use tokio::io::AsyncWriteExt as _;

use crate::{
    chat_message::ChatMessage,
    commands::{Command, CommandEvent, CommandResponse, Responder},
    frontend::{ui_event::UIEvent, App},
    git,
};

// Shows a diff to the user
pub async fn diff_show(app: &mut App<'_>) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let current_chat_uuid = app.current_chat_uuid;

    let event = CommandEvent::builder()
        .command(Command::Diff)
        .uuid(current_chat_uuid)
        .responder(Arc::new(tx))
        .build()
        .expect("Infallible; should not fail to build event for diff show");

    app.dispatch_command_event(event);

    // App tx so we forward everything else
    // TODO: Think of a nicer way to do this. It's a bit hacky. Maybe a forwarder?
    let app_tx = app.command_responder.for_chat_id(current_chat_uuid);
    let mut diff_message = String::new();
    while let Some(msg) = rx.recv().await {
        match msg {
            CommandResponse::BackendMessage(ref payload) => {
                if diff_message.is_empty() {
                    diff_message = payload.to_string();
                    let rendered = ansi_to_tui::IntoText::into_text(&diff_message).ok();

                    app.send_ui_event(UIEvent::ChatMessage(
                        current_chat_uuid,
                        ChatMessage::new_system(diff_message.clone())
                            .with_rendered(rendered)
                            .to_owned(),
                    ));
                } else {
                    app_tx.send(msg).await;
                }
            }
            CommandResponse::Completed => {
                app_tx.send(msg).await;
                break;
            }
            _ => app_tx.send(msg).await,
        }
    }
}

// Pulls the diff from the backend as a patch and applies it to the same branch as the agent is running in
#[allow(clippy::too_many_lines)]
pub async fn diff_pull(app: &mut App<'_>) {
    // if the local current branch is dirty, we should not pull
    // Maybe we can move these to util, and then use a local executor
    if git::util::is_dirty(&app.workdir).await {
        app.send_ui_event(UIEvent::ChatMessage(
            app.current_chat_uuid,
            ChatMessage::new_system("Cannot pull diff, working directory is dirty".to_string()),
        ));
        return;
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let mut branch = String::new();
    let current_chat_uuid = app.current_chat_uuid;
    let branch_event = CommandEvent::builder()
        .command(Command::Exec {
            cmd: swiftide::traits::Command::shell("git rev-parse --abbrev-ref HEAD"),
        })
        .uuid(current_chat_uuid)
        .responder(Arc::new(tx.clone()))
        .build()
        .expect("Infallible; should not fail to build event for branch");

    app.dispatch_command_event(branch_event);

    while let Some(msg) = rx.recv().await {
        match msg {
            CommandResponse::BackendMessage(ref payload) => {
                if branch.is_empty() {
                    branch = payload.to_string();
                }
            }
            CommandResponse::Completed => {
                break;
            }
            _ => (),
        }
    }

    tracing::debug!("Current branch: {:?}", branch);

    let current_chat_uuid = app.current_chat_uuid;

    let diff_event = CommandEvent::builder()
        .command(Command::Diff)
        .uuid(current_chat_uuid)
        .responder(Arc::new(tx))
        .build()
        .expect("Infallible; should not fail to build event for diff show");

    app.dispatch_command_event(diff_event);

    // App tx so we forward everything else
    let mut diff = String::new();
    while let Some(msg) = rx.recv().await {
        match msg {
            CommandResponse::BackendMessage(ref payload) => {
                if diff.is_empty() {
                    diff = payload.to_string();
                }
            }
            CommandResponse::Completed => {
                break;
            }
            _ => (),
        }
    }

    tracing::debug!("Diff: {:?}", diff);
    // check out the branch, then apply the diff
    let output = tokio::process::Command::new("git")
        .arg("checkout")
        .arg("-b")
        .arg(branch.trim())
        .current_dir(&app.workdir)
        .output()
        .await
        .expect("Failed to checkout branch");

    tracing::debug!("Checkout output: {:?}", output);

    if !output.status.success() {
        app.send_ui_event(UIEvent::ChatMessage(
            current_chat_uuid,
            ChatMessage::new_system("Failed to checkout branch".to_string()),
        ));

        app.command_responder
            .for_chat_id(current_chat_uuid)
            .send(CommandResponse::Completed)
            .await;
        return;
    }

    // // Makes stdin happy
    diff.push('\n');

    // Apply the patch
    let mut process = tokio::process::Command::new("git")
        .arg("apply")
        .arg("-")
        .current_dir(&app.workdir)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn git apply");

    process
        .stdin
        .take()
        .expect("Failed to get stdin")
        .write_all(&strip_ansi_escapes::strip(diff))
        .await
        .expect("Failed to write diff to git apply");

    let output = process
        .wait_with_output()
        .await
        .expect("Failed to wait for git apply");

    if !output.status.success() {
        let error =
            String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);

        app.send_ui_event(UIEvent::ChatMessage(
            current_chat_uuid,
            ChatMessage::new_system(format!("Failed to apply diff: {error}")),
        ));

        app.command_responder
            .for_chat_id(current_chat_uuid)
            .send(CommandResponse::Completed)
            .await;
        return;
    }
    // add all changes
    let output = tokio::process::Command::new("git")
        .arg("add")
        .arg(".")
        .current_dir(&app.workdir)
        .output()
        .await
        .expect("Failed to add changes");
    tracing::debug!("Add output: {:?}", output);

    // Commit the patch
    let output = tokio::process::Command::new("git")
        .arg("commit")
        .arg("-am")
        .arg("Applied diff")
        .current_dir(&app.workdir)
        .output()
        .await
        .expect("Failed to commit diff");
    tracing::debug!("Commit output: {:?}", output);

    // Check out the previous branch
    let output = tokio::process::Command::new("git")
        .arg("checkout")
        .arg("-")
        .current_dir(&app.workdir)
        .output()
        .await
        .expect("Failed to checkout previous branch");
    tracing::debug!("Checkout previous branch output: {:?}", output);

    // Send a message to the user with the branch name
    app.send_ui_event(UIEvent::ChatMessage(
        current_chat_uuid,
        ChatMessage::new_system(format!("Pulled diff into branch `{branch}`")),
    ));

    // Tell the app that we are done
    app.command_responder
        .for_chat_id(current_chat_uuid)
        .send(CommandResponse::Completed)
        .await;
}
