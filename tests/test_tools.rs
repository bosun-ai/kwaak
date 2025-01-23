use std::sync::Arc;

use kwaak::agent::tools;
use serde_json::json;
use swiftide::agents::{tools::local_executor::LocalExecutor, DefaultContext};
use swiftide_core::{AgentContext, ToolExecutor};
use tempfile::tempdir;

macro_rules! invoke {
    // Takes the context and the json value
    // Returns the result
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
    let executor = LocalExecutor::builder()
        .workdir(env!("CARGO_MANIFEST_DIR"))
        .build()
        .unwrap();

    DefaultContext::from_executor(Arc::new(executor) as Arc<dyn ToolExecutor>)
}

#[test_log::test(tokio::test)]
async fn test_search_file() {
    let tool = tools::search_file();
    let context = setup_context();

    // list dirs on empty
    let list_result = invoke!(&tool, &context, json!({"file_name": "."}));

    assert!(list_result.contains("tests"));
    assert!(list_result.contains("src"));

    eprintln!("{list_result}");

    // Ensure we never list everything in the git dir
    // as that is a lot of tokens
    assert!(
        list_result
            .split('\n')
            .filter(|f| f.starts_with(".git/"))
            .count()
            <= 1,
    );
    assert!(list_result.contains(".github"));

    // search with path
    let with_path = invoke!(&tool, &context, json!({"file_name": "src"}));

    assert!(with_path.contains("src/main.rs"));

    // search single file (no path)
    let with_single_file = invoke!(&tool, &context, json!({"file_name": "main.rs"}));

    assert!(with_single_file.contains("src/main.rs"));

    // with single file and path
    let with_single_file_and_path = invoke!(&tool, &context, json!({"file_name": "src/main.rs"}));

    assert!(with_single_file_and_path.contains("src/main.rs"));

    // Always case insensitive
    let with_case_insensitive = invoke!(&tool, &context, json!({"file_name": "MaIn.Rs"}));

    assert!(with_case_insensitive.contains("src/main.rs"));
}

#[test_log::test(tokio::test)]
async fn test_search_code() {
    let tool = tools::search_code();
    let context = setup_context();

    // includes hidden
    let include_hidden = invoke!(&tool, &context, json!({"query": "first-line-heading"}));

    assert!(include_hidden.contains(".markdownlint.yaml"));

    // always ignores case
    let case_insensitive = invoke!(&tool, &context, json!({"query": "First-Line-HEADING"}));
    assert!(case_insensitive.contains(".markdownlint.yaml"));

    // Should only do literal searches
    let literal_search = invoke!(&tool, &context, json!({"query": "[test_search_code]"}));
    assert!(literal_search.lines().count() < 3);
    assert!(literal_search.contains("test_tools.rs"));
}

#[test_log::test(tokio::test)]
async fn test_replace_block() {
    let tool = tools::replace_block();
    let context = setup_context();

    let tempdir = tempdir().unwrap();
    // create a fake file with multiple lines
    std::fs::write(
        tempdir.path().join("test.txt"),
        "line1\nline2\nline3\nline4\nline5",
    )
    .unwrap();
    // then invoke the tool
    let tool_response = invoke!(
        &tool,
        &context,
        json!({
            "file_name": tempdir.path().join("test.txt").to_str().unwrap(),
            "start_line": "2",
            "end_line": "4",
            "replacement": "one line"
        })
    );

    let new_file_content = std::fs::read_to_string(tempdir.path().join("test.txt")).unwrap();

    assert_eq!(new_file_content, "line1\none line\nline5");
    assert!(tool_response.contains("Successfully replaced block"));

    // Replacing multiple lines with multiple lines
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
            "end_line": "4",
            "replacement": "one\nline"
        })
    );

    assert!(tool_response.contains("Successfully replaced block"));
    assert_eq!(
        std::fs::read_to_string(tempdir.path().join("test.txt")).unwrap(),
        "line1\none\nline\nline5"
    );

    // Index out of bounds
    //
    let tool_response = invoke!(
        &tool,
        &context,
        json!({
            "file_name": tempdir.path().join("test.txt").to_str().unwrap(),
            "start_line": "2",
            "end_line": "10",
            "replacement": "one\nline"
        })
    );

    assert!(
        tool_response.contains("Start or end line number is out of bounds"),
        "{}",
        &tool_response
    );

    // File not found
    let tool_args = json!({
        "file_name": tempdir.path().join("test2.txt").to_str().unwrap(),
        "start_line": "2",
        "end_line": "4",
        "replacement": "one\nline"
    });

    let tool_response = tool
        .invoke(&context as &dyn AgentContext, Some(&tool_args.to_string()))
        .await
        .unwrap_err();

    assert!(
        tool_response
            .to_string()
            .contains("No such file or directory"),
        "actual: {}",
        &tool_response.to_string()
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

    let expected = "1: line1\n2: line2\n3: line3\n4: line4\n5: line5";
    assert_eq!(file_content, expected);
}
