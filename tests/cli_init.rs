use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::TempDir;
struct Context {
    cmd: Command,
    dir: TempDir,
}

fn setup() -> Context {
    let dir = tempfile::Builder::new()
        .prefix("kwaak-test")
        .tempdir()
        .unwrap();

    let mut cmd = Command::cargo_bin("kwaak").unwrap();
    cmd.arg("init").current_dir(&dir);

    Context { cmd, dir }
}

#[test_log::test(tokio::test)]
async fn test_creates_a_new_init_file() {
    let mut context = setup();
    Command::new("git")
        .arg("init")
        .current_dir(&context.dir)
        .assert()
        .success();

    // Add a remote
    Command::new("git")
        .args([
            "remote",
            "add",
            "origin",
            "https://github.com/bosun-ai/kwaak",
        ])
        .current_dir(&context.dir)
        .assert()
        .success();

    context
        .cmd
        .assert()
        .stdout(predicate::str::contains("Initialized kwaak project"))
        .success();

    // assert the file exists
    std::fs::metadata(context.dir.path().join("kwaak.toml")).unwrap();
}

#[test_log::test(tokio::test)]
async fn test_fails_if_not_git() {
    let mut context = setup();
    context
        .cmd
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test_log::test(tokio::test)]
async fn test_fails_config_present() {
    let mut cmd = Command::cargo_bin("kwaak").unwrap();
    cmd.arg("init");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}
