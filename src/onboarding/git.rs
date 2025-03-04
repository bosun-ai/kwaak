use anyhow::Result;
use serde_json::json;

use crate::{
    config::defaults::{default_main_branch, default_owner_and_repo},
    onboarding::util::{prompt_api_key, prompt_text},
};

pub fn git_questions(context: &mut tera::Context) -> Result<()> {
    let (default_owner, default_repository) = default_owner_and_repo().unzip();
    let default_branch = default_main_branch();
    let branch_input = prompt_text("Default git branch", Some(&default_branch)).prompt()?;

    println!(
        "\nWith a github token, Kwaak can create pull requests, retrieve and work on issues, search
        github code, and automatically push to the remote. Kwaak will never push to the main branch."
    );

    let github_api_key = prompt_api_key("Github token (optional, <esc> to skip)", None)
        .with_placeholder("env:GITHUB_token")
        .prompt_skippable()?;

    let auto_push_remote = if github_api_key.is_some() {
        inquire::Confirm::new("Push to git remote after changes? (requires github token)")
            .with_default(false)
            .prompt()?
    } else {
        false
    };

    context.insert("github_api_key", &github_api_key);
    context.insert(
        "git",
        &json!({
            "owner": default_owner,
            "repository": default_repository,
            "main_branch": branch_input,
            "auto_push_remote": auto_push_remote,

        }),
    );

    Ok(())
}
