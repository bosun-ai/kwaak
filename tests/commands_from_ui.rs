use kwaak::commands::{Command, CommandHandler};
use kwaak::frontend::{ui, App, DiffVariant, UIEvent, UserInputCommand};
use kwaak::{storage, test_utils};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use swiftide_core::Persist;
use uuid::Uuid;

/// Macro to wait for a command to be done
macro_rules! assert_command_done {
    ($app:expr, $uuid:expr) => {
        let event = $app
            .handle_events_until(UIEvent::is_command_done)
            .await
            .unwrap();

        assert_eq!(event, UIEvent::CommandDone($uuid));
    };
}

/// Tests showing the diff of an agent workspace, and then pulling the diff into a local branch
#[test_log::test(tokio::test(flavor = "multi_thread"))]
async fn test_diff() {
    // TODO: Currently not working on CI, complains it's not a git dir
    if std::env::var("CI").is_ok() {
        return;
    }
    let (repository, _guard) = test_utils::test_repository();
    let workdir = repository.path().clone();
    let mut app = App::default().with_workdir(repository.path());
    let lancedb = storage::get_lancedb(&repository);
    lancedb.setup().await.unwrap();
    let mut terminal = Terminal::new(TestBackend::new(160, 40)).unwrap();

    let mut handler = CommandHandler::from_repository(repository);
    handler.register_ui(&mut app);
    let _handler_guard = handler.start();

    let fixed_uuid = Uuid::parse_str("936DA01F9ABD4d9d80C702AF85C822A8").unwrap();
    let Some(current_chat) = app.current_chat_mut() else {
        panic!("No current chat");
    };

    // Force to fixed uuid so that snapshots are stable
    current_chat.uuid = fixed_uuid;
    app.current_chat_uuid = fixed_uuid;

    // First, let's start a noop agent so an environment is running
    app.dispatch_command(
        fixed_uuid,
        Command::Chat {
            message: "hello".to_string(),
        },
    );

    assert_command_done!(app, fixed_uuid);

    // The user asks for a diff, it should be empty
    app.send_ui_event(UIEvent::UserInputCommand(
        fixed_uuid,
        UserInputCommand::Diff(DiffVariant::Show),
    ));

    assert_command_done!(app, fixed_uuid);

    // Now let's add a file and check the diff
    app.dispatch_command(
        fixed_uuid,
        Command::Exec {
            cmd: swiftide::traits::Command::write_file("hello.txt", "world"),
        },
    );

    assert_command_done!(app, fixed_uuid);

    // now get the diff
    app.send_ui_event(UIEvent::UserInputCommand(
        fixed_uuid,
        UserInputCommand::Diff(DiffVariant::Show),
    ));

    assert_command_done!(app, fixed_uuid);

    terminal.draw(|f| ui(f, f.area(), &mut app)).unwrap();
    insta::assert_snapshot!(terminal.backend());

    // let's pull the diff
    app.send_ui_event(UIEvent::UserInputCommand(
        fixed_uuid,
        UserInputCommand::Diff(DiffVariant::Pull),
    ));

    assert_command_done!(app, fixed_uuid);

    // First check that the current branch is still main
    let current_branch = tokio::process::Command::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(&workdir)
        .output()
        .await
        .unwrap();

    let current_branch = std::str::from_utf8(&current_branch.stdout).unwrap().trim();
    assert_eq!(current_branch, "main");
    // Now let's check out the branch and verify we have the hello.txt
    let output = tokio::process::Command::new("git")
        .arg("checkout")
        .arg(format!("kwaak/{fixed_uuid}"))
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
}
