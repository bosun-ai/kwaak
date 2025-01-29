use assert_cmd::{cargo::cargo_bin, prelude::*};
use kwaak::test_utils::temp_env;
use predicates::prelude::*;
use rexpect::{process::wait::WaitStatus, spawn};
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
        cmd.env("KWAAK_GITHUB_TOKEN", "noop");
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

    fn commit_changes(self) -> Self {
        Command::new("git")
            .args(["add", "."])
            .current_dir(&self.dir)
            .assert()
            .success();
        Command::new("git")
            .args(["commit", "-m", "Test commit"])
            .current_dir(&self.dir)
            .assert()
            .success();

        self
    }
}

#[test_log::test(tokio::test)]
async fn test_interactive_default_init() {
    let _temp_openai_api_key = temp_env("OPENAI_API_KEY", "noop");
    let _temp_github_token = temp_env("GITHUB_TOKEN", "noop");
    let cmd = cargo_bin("kwaak");

    let mut p = spawn(&format!("{cmd:?} init --dry-run"), Some(30_000)).unwrap();

    while let Ok(line) = p.read_line() {
        println!("{line}");
        // if line.contains("Dry run, would have written") {
        //     break;
        // }
        if line.contains("base url") {
            let _ = p.send_line("https://api.bosun.ai");
        } else {
            let _ = p.send_line("");
        }
    }

    println!("{}", p.exp_eof().unwrap());

    let Ok(WaitStatus::Exited(.., status)) = p.process.wait() else {
        panic!("Process had a weird exit status");
    };
    assert_eq!(status, 0);
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
    let mut context = setup().with_git().with_config().commit_changes();

    context.cmd().arg("print-config").assert().success();
}

#[test_log::test(tokio::test)]
async fn test_self_fixing_after_clear_cache() {
    let mut context = setup().with_git().with_config().commit_changes();

    context.cmd().arg("clear-cache").assert().success();
    context.cmd().arg("print-config").assert().success();
}
