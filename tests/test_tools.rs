use kwaak::agent::tools;
use serde_json::json;
use swiftide::agents::DefaultContext;
use swiftide_core::AgentContext;
use swiftide_docker_executor::DockerExecutor;

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

async fn setup_context() -> DefaultContext {
    let executor = DockerExecutor::default()
        .with_image_name("test")
        .to_owned()
        .start()
        .await
        .unwrap();

    DefaultContext::from_executor(executor)
}

#[test_log::test(tokio::test)]
async fn test_search_file() {
    let tool = tools::search_file();
    let context = setup_context().await;

    // list dirs on empty
    let list_result = invoke!(&tool, &context, json!({"file_name": "."}));

    assert!(list_result.contains("tests"));
    assert!(list_result.contains("src"));

    // include hidden
    assert!(list_result.contains(".git"));
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
    let context = setup_context().await;

    // includes hidden
    let include_hidden = invoke!(&tool, &context, json!({"query": "runs-on"}));

    assert!(include_hidden.contains(".github/workflows"));

    // always ignores case
    let case_insensitive = invoke!(&tool, &context, json!({"query": "RuNs-On"}));
    assert!(case_insensitive.contains(".github/workflows"));

    // Should only do literal searches
    let literal_search = invoke!(&tool, &context, json!({"query": "[test_search_code]"}));
    assert!(literal_search.lines().count() < 3);
    assert!(literal_search.contains("test_tools.rs"));
}
