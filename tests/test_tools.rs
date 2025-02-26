use std::sync::Arc;

use kwaak::agent::tools;
use serde_json::json;
use swiftide::agents::{DefaultContext, tools::local_executor::LocalExecutor};
use swiftide_core::{AgentContext, ToolExecutor};
use tempfile::tempdir;

macro_rules! invoke {
    ($tool:expr, $context:expr, $json:expr) => {{
        let json = $json.to_string();

        $tool
            .invoke($context as &dyn AgentContext, Some(&json))
            .await
            .unwrap()
            .content()
            .unwrap()
            .to_string()
    }};
}

fn setup_context() -> DefaultContext {
    let executor = LocalExecutor::builder().build().unwrap();

    DefaultContext::from_executor(Arc::new(executor) as Arc<dyn ToolExecutor>)
}

#[test_log::test(tokio::test)]
async fn test_search_file() {
    let tool = tools::search_file();
    let context = setup_context();

    let list_result = invoke!(&tool, &context, json!({"file_name": "."}));

    assert!(list_result.contains("tests"));
    assert!(list_result.contains("src"));

    eprintln!("{list_result}");

    assert!(
        list_result
            .split('\n')
            .filter(|f| f.starts_with(".git/"))
            .count()
            <= 1,
    );
    assert!(list_result.contains(".github"));

    let with_path = invoke!(&tool, &context, json!({"file_name": "src"}));

    assert!(with_path.contains("src/main.rs"));

    let with_single_file = invoke!(&tool, &context, json!({"file_name": "main.rs"}));

    assert!(with_single_file.contains("src/main.rs"));

    let with_single_file_and_path = invoke!(&tool, &context, json!({"file_name": "src/main.rs"}));

    assert!(with_single_file_and_path.contains("src/main.rs"));

    let with_case_insensitive = invoke!(&tool, &context, json!({"file_name": "MaIn.Rs"}));

    assert!(with_case_insensitive.contains("src/main.rs"));
}

#[test_log::test(tokio::test)]
async fn test_search_code() {
    let tool = tools::search_code();
    let context = setup_context();

    // These don't work on CI, but fine locally and in docker
    // Not a clue why suddenly. Coverage still runs them fine
    if std::env::var("CI").is_ok() {
        return;
    }
    let literal_search = invoke!(&tool, &context, json!({"query": "[test_search_code]"}));
    dbg!(&literal_search);
    assert!(literal_search.lines().count() < 3);
    assert!(literal_search.contains("test_tools.rs"));

    let main = invoke!(&tool, &context, json!({"query": "main"}));
    assert!(main.contains("main.rs"));

    let include_hidden = invoke!(&tool, &context, json!({"query": "first-line-heading"}));

    dbg!(&include_hidden);
    assert!(include_hidden.contains(".markdownlint.yaml"));

    let case_insensitive = invoke!(&tool, &context, json!({"query": "First-Line-HEADING"}));
    dbg!(&case_insensitive);
    assert!(case_insensitive.contains(".markdownlint.yaml"));
}

#[test_log::test(tokio::test)]
async fn test_edit_file() {
    let tool = tools::replace_lines();
    let context = setup_context();

    let tempdir = tempdir().unwrap();
    std::fs::write(
        tempdir.path().join("test.txt"),
        "line1\nline2\nline3\nline4\nline5",
    )
    .unwrap();

    let tool_response = invoke!(
        &tool,
        &context,
        json!({
            "file_name": tempdir.path().join("test.txt").to_str().unwrap(),
            "start_line": "2",
            "end_line": "5",
            "content": "line2\none line\nline5"
        })
    );

    let new_file_content = std::fs::read_to_string(tempdir.path().join("test.txt")).unwrap();

    assert_eq!(new_file_content, "line1\nline2\none line\nline5");
    assert!(tool_response.contains("Successfully replaced content"));

    std::fs::write(
        tempdir.path().join("test.txt"),
        "line1\nline2\nline3\nline4\nline5",
    )
    .unwrap();

    let tool_response = invoke!(
        &tool,
        &context,
        json!({
            "file_name": tempdir.path().join("test.txt").to_str().unwrap(),
            "start_line": "1",
            "end_line": "4",
            "content": "line1\none\nline\nline4"
        })
    );

    assert!(tool_response.contains("Successfully replaced content"));
    assert_eq!(
        std::fs::read_to_string(tempdir.path().join("test.txt")).unwrap(),
        "line1\none\nline\nline4\nline5"
    );

    let tool_response = invoke!(
        &tool,
        &context,
        json!({
            "file_name": tempdir.path().join("test.txt").to_str().unwrap(),
            "start_line": "2",
            "end_line": "10",
            "content": "one\nline"
        })
    );

    assert!(
        tool_response.contains("Start or end line number is out of bounds"),
        "{}",
        &tool_response
    );

    let tool_args = json!({
        "file_name": tempdir.path().join("test2.txt").to_str().unwrap(),
        "start_line": "2",
        "end_line": "4",
        "content": "one\nline"
    });

    let tool_response = tool
        .invoke(&context as &dyn AgentContext, Some(&tool_args.to_string()))
        .await
        .unwrap();

    assert!(
        tool_response
            .to_string()
            .contains("No such file or directory"),
        "actual: {}",
        &tool_response.to_string()
    );

    // Test replacing with mismatched indentation
    std::fs::write(
        tempdir.path().join("test.txt"),
        indoc::indoc! {"
        def a:

            def b:
                return True

            return True

        def c:
            return True
        "},
    )
    .unwrap();

    let tool_response = invoke!(
        &tool,
        &context,
        json!({
            "file_name": tempdir.path().join("test.txt").to_str().unwrap(),
            "start_line": "3",
            "end_line": "6",
            "content": indoc::indoc! {"
                def b:
                    return False

                return True
            "}
        })
    );

    assert!(tool_response.contains("Successfully replaced content"));
    assert_eq!(
        std::fs::read_to_string(tempdir.path().join("test.txt")).unwrap(),
        indoc::indoc! {"
            def a:

                def b:
                    return False

                return True

            def c:
                return True
        "},
    );
}

#[test_log::test(tokio::test)]
async fn test_read_file_with_line_numbers() {
    let tool = tools::read_file_with_line_numbers();
    let context = setup_context();

    let tempdir = tempdir().unwrap();
    std::fs::write(
        tempdir.path().join("test.txt"),
        "line1\nline2\nline3\nline4\nline5",
    )
    .unwrap();

    let file_content = invoke!(
        &tool,
        &context,
        json!({
            "file_name": tempdir.path().join("test.txt").to_str().unwrap(),
        })
    );

    let expected = "1|line1\n2|line2\n3|line3\n4|line4\n5|line5";
    assert_eq!(file_content, expected);
}

#[test_log::test(tokio::test)]
async fn test_read_file() {
    let tool = tools::read_file();
    let context = setup_context();

    let tempdir = tempdir().unwrap();
    std::fs::write(tempdir.path().join("test.txt"), "line1\nline2\nline3").unwrap();

    let file_content = invoke!(
        &tool,
        &context,
        json!({"file_name": tempdir.path().join("test.txt").to_str().unwrap()})
    );

    let expected_content = "line1\nline2\nline3";
    assert_eq!(file_content, expected_content);
}

#[test_log::test(tokio::test)]
async fn test_write_file() {
    let tool = tools::write_file();
    let context = setup_context();

    let tempdir = tempdir().unwrap();
    let file_path = tempdir.path().join("test.txt");
    let content = "new content";

    let tool_response = invoke!(
        &tool,
        &context,
        json!({
            "file_name": file_path.to_str().unwrap(),
            "content": content
        })
    );

    let written_content = std::fs::read_to_string(file_path).unwrap();

    assert_eq!(written_content, content);
    assert!(tool_response.contains("File written successfully"));
}

#[tokio::test]
async fn test_shell_command() {
    let tool = tools::shell_command();
    let context = setup_context();

    let command_output = invoke!(&tool, &context, json!({"cmd": "echo 'test'"}));

    assert!(command_output.contains("test"));
}

#[tokio::test]
async fn test_git() {
    let tool = tools::git();
    let context = setup_context();

    let git_output = invoke!(&tool, &context, json!({"command": "status"}));

    dbg!(&git_output);
    assert!(git_output.contains("On branch") || git_output.contains("HEAD detached"));
}
