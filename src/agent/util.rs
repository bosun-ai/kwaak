use anyhow::Context as _;
use anyhow::Result;
use swiftide_core::SimplePrompt;
use uuid::Uuid;

use crate::commands::Responder;

pub async fn rename_chat(
    query: &str,
    fast_query_provider: &dyn SimplePrompt,
    command_responder: &dyn Responder,
) -> Result<()> {
    let chat_name = fast_query_provider
        .prompt(
            format!("Give a good, short, max 60 chars title for the following query. Only respond with the title.:\n{query}")
                .into(),
        )
        .await
        .context("Could not get chat name")?
        .trim_matches('"')
        .chars()
        .take(60)
        .collect::<String>();

    command_responder.rename_chat(&chat_name);

    Ok(())
}

pub async fn create_branch_name(
    query: &str,
    uuid: &Uuid,
    fast_query_provider: &dyn SimplePrompt,
    command_responder: &dyn Responder,
) -> Result<String> {
    let name = fast_query_provider
        .prompt(
            format!("Give a good, short, max 30 chars git-branch-name for the following query. Only respond with the git-branch-name.:\n{query}")
                .into(),
        )
        .await
        .context("Could not get chat name")?
        .trim_matches('"')
        .chars()
        .take(30)
        .collect::<String>();

    // only keep ascii characters
    let name = name.chars().filter(char::is_ascii).collect::<String>();
    let name = name.to_lowercase();

    // replace all non-alphanumeric characters with dashes
    let name = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();

    // get the first 8 characters of the uuid
    let uuid_start = uuid.to_string().chars().take(8).collect::<String>();
    let branch_name = format!("kwaak/{name}-{uuid_start}");

    command_responder.rename_branch(&branch_name);

    Ok(branch_name)
}

#[cfg(test)]
mod tests {
    use swiftide_core::MockSimplePrompt;

    use crate::commands::MockResponder;
    use mockall::{predicate, PredicateBooleanExt};

    use super::*;

    #[tokio::test]
    async fn test_rename_chat() {
        let query = "This is a query";
        let mut llm_mock = MockSimplePrompt::new();
        llm_mock
            .expect_prompt()
            .returning(|_| Ok("Excellent title".to_string()));

        let mut mock_responder = MockResponder::default();

        mock_responder
            .expect_rename_chat()
            .with(predicate::eq("Excellent title"))
            .once()
            .returning(|_| ());

        rename_chat(&query, &llm_mock as &dyn SimplePrompt, &mock_responder)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rename_chat_limits_60() {
        let query = "This is a query";
        let mut llm_mock = MockSimplePrompt::new();
        llm_mock
            .expect_prompt()
            .returning(|_| Ok("Excellent title".repeat(100).to_string()));

        let mut mock_responder = MockResponder::default();

        mock_responder
            .expect_rename_chat()
            .with(
                predicate::str::starts_with("Excellent title")
                    .and(predicate::function(|s: &str| s.len() == 60)),
            )
            .once()
            .returning(|_| ());

        rename_chat(&query, &llm_mock as &dyn SimplePrompt, &mock_responder)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rename_branch() {
        let query = "This is a query";
        let mut llm_mock = MockSimplePrompt::new();
        llm_mock
            .expect_prompt()
            .returning(|_| Ok("excellent-name".to_string()));

        let mut mock_responder = MockResponder::default();
        let fixed_uuid = Uuid::parse_str("936DA01F9ADD4d9d80C702AF85C822A8").unwrap();

        mock_responder
            .expect_rename_branch()
            .with(predicate::str::starts_with("kwaak/excellent-name"))
            .once()
            .returning(|_| ());

        create_branch_name(
            &query,
            &fixed_uuid,
            &llm_mock as &dyn SimplePrompt,
            &mock_responder,
        )
        .await
        .unwrap();
    }

    // NOTE the prompt is intended to be limited to 30 characters, but the branch name in total
    // has 15 more characters (total 45): "kwaak/" + "-" + 8 characters from the uuid
    #[tokio::test]
    async fn test_rename_branch_limits_45() {
        let query = "This is a query";
        let mut llm_mock = MockSimplePrompt::new();
        llm_mock
            .expect_prompt()
            .returning(|_| Ok("excellent-name".repeat(100).to_string()));

        let mut mock_responder = MockResponder::default();
        let fixed_uuid = Uuid::parse_str("936DA01F9ADD4d9d80C702AF85C822A8").unwrap();

        mock_responder
            .expect_rename_branch()
            .with(
                predicate::str::starts_with("kwaak/excellent-name")
                    .and(predicate::function(|s: &str| s.len() == 45)),
            )
            .once()
            .returning(|_| ());

        create_branch_name(
            &query,
            &fixed_uuid,
            &llm_mock as &dyn SimplePrompt,
            &mock_responder,
        )
        .await
        .unwrap();
    }
}
