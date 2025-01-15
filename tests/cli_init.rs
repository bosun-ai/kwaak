use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use tempfile::TempDir;
struct Context {
    dir: TempDir,
}

fn setup() -> Context {
    let dir = tempfile::Builder::new()
        .prefix("kwaak-test")
        .tempdir()
        .unwrap();

    Context { dir }
}

impl Context {
    fn cmd(&mut self) -> Command {
        let mut cmd = Command::cargo_bin("kwaak").unwrap();
        cmd.current_dir(&self.dir);
        cmd.env_clear();
        cmd.env("TAVILY_API_KEY", "noop");
        cmd.env("GITHUB_TOKEN", "noop");
        cmd.env("KWAAK_OPENAI_API_KEY", "noop");
        cmd.env("RUST_LOG", "debug");
        cmd.env("RUST_BACKTRACE", "1");
        cmd
    }

    fn with_git(self) -> Self {
        Command::new("git")
            .arg("init")
            .current_dir(&self.dir)
            .assert()
            .success();

        Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/bosun-ai/kwaak",
            ])
            .current_dir(&self.dir)
            .assert()
            .success();

        self
    }

    fn with_config(self) -> Self {
        // Copies over kwaak.toml to the tempdir
        Command::new("cp")
            .args(["kwaak.toml", self.dir.path().to_str().unwrap()])
            .assert()
            .success();

        self
    }
}

#[test_log::test(tokio::test)]
async fn test_creates_a_new_init_file() {
    let mut context = setup().with_git();

    context
        .cmd()
        .arg("init")
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
        .cmd()
        .arg("init")
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

#[test_log::test(tokio::test)]
async fn test_print_config() {
    let mut context = setup().with_git().with_config();

    context.cmd().arg("print-config").assert().success();
}

#[test_log::test(tokio::test)]
async fn test_self_fixing_after_clear_cache() {
    let mut context = setup().with_git().with_config();

    context.cmd().arg("clear-cache").assert().success();
    context.cmd().arg("print-config").assert().success();
}
