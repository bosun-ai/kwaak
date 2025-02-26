use kwaak::commands::Command;
use kwaak::frontend::{DiffVariant, UIEvent, UserInputCommand, ui};
use kwaak::test_utils::{IntegrationContext, setup_integration};
use kwaak::{assert_command_done, git};

/// Tests showing the diff of an agent workspace, and then pulling the diff into a local branch
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn test_diff() {
    let IntegrationContext {
        mut app,
        uuid,
        repository,
        mut terminal,
        workdir,

        repository_guard: _repository_guard,
        handler_guard,
    } = setup_integration().await.unwrap();

    // First, let's start a noop agent so an environment is running
    app.dispatch_command(
        uuid,
        Command::Chat {
            message: "hello".to_string(),
        },
    );

    assert_command_done!(app, uuid);

    // The user asks for a diff, it should be empty
    app.send_ui_event(UIEvent::UserInputCommand(
        uuid,
        UserInputCommand::Diff(DiffVariant::Show),
    ));

    assert_command_done!(app, uuid);

    // Now let's add a file and check the diff
    app.dispatch_command(
        uuid,
        Command::Exec {
            cmd: swiftide::traits::Command::write_file("hello.txt", "world"),
        },
    );

    assert_command_done!(app, uuid);

    // now get the diff
    app.send_ui_event(UIEvent::UserInputCommand(
        uuid,
        UserInputCommand::Diff(DiffVariant::Show),
    ));

    assert_command_done!(app, uuid);

    terminal.draw(|f| ui(f, f.area(), &mut app)).unwrap();
    insta::assert_snapshot!(terminal.backend());

    // let's pull the diff
    app.send_ui_event(UIEvent::UserInputCommand(
        uuid,
        UserInputCommand::Diff(DiffVariant::Pull),
    ));

    assert_command_done!(app, uuid);

    let current_branch = git::util::main_branch(&workdir);
    assert_eq!(&current_branch, &repository.config().git.main_branch);

    let branch_name = if let Some(chat) = app.current_chat() {
        chat.branch_name.clone().expect("branch not yet named")
    } else {
        panic!("No current chat");
    };

    // Now let's check out the branch and verify we have the hello.txt
    let output = tokio::process::Command::new("git")
        .arg("checkout")
        .arg(&branch_name)
        .current_dir(&workdir)
        .output()
        .await
        .unwrap();
    dbg!(&output);

    let output = tokio::process::Command::new("git")
        .arg("status")
        .current_dir(&workdir)
        .output()
        .await
        .unwrap();

    dbg!(&output);

    let output = tokio::process::Command::new("git")
        .arg("branch")
        .current_dir(&workdir)
        .output()
        .await
        .unwrap();
    dbg!(&output);

    // And read the file
    let content = std::fs::read_to_string(workdir.join("hello.txt")).unwrap();

    assert_eq!(content, "world\n");

    app.handle_single_event(&UIEvent::ScrollDown).await;

    terminal.draw(|f| ui(f, f.area(), &mut app)).unwrap();
    insta::assert_snapshot!("diff pulled", terminal.backend());

    drop(handler_guard);
}
