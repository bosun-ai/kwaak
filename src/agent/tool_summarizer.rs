use std::sync::Arc;

use swiftide::{
    agents::hooks::AfterToolFn,
    chat_completion::{Tool, ToolCall, ToolOutput},
    prompt::Prompt,
    traits::{Command, SimplePrompt},
};
use tracing::Instrument as _;

use crate::{templates::Templates, util::accept_non_zero_exit};

#[derive(Clone)]
pub struct ToolSummarizer<'a> {
    llm: Arc<dyn SimplePrompt>,
    tools_to_summarize: Vec<&'a str>,
    available_tools: Vec<Box<dyn Tool>>,
    git_start_sha: String,
}

impl<'a> ToolSummarizer<'a> {
    pub fn new(
        llm: Box<dyn SimplePrompt>,
        tools_to_summarize: &[&'a str],
        available_tools: &[Box<dyn Tool>],
        git_start_sha: impl Into<String>,
    ) -> Self {
        Self {
            llm: llm.into(),
            tools_to_summarize: tools_to_summarize.into(),
            available_tools: available_tools.into(),
            git_start_sha: git_start_sha.into(),
        }
    }

    // Rust has a long outstanding issue where it captures outer lifetimes when returning an impl
    // that also has lifetimes, see https://github.com/rust-lang/rust/issues/42940
    #[must_use]
    pub fn summarize_hook(self) -> impl AfterToolFn + use<'a> {
        move |agent, tool_call, tool_output| {
            let llm = self.llm.clone();

            let Some(tool) = self
                .tools_to_summarize
                .iter()
                .find(|t| *t == &tool_call.name())
                .and_then(|t| self.available_tools.iter().find(|tool| &tool.name() == t))
            else {
                return Box::pin(async move { Ok(()) });
            };

            if let Ok(output) = tool_output {
                let git_start_sha = self.git_start_sha.clone();

                let span = tracing::info_span!("summarize_tool", tool = tool.name().as_ref());
                let prompt = prompt(tool, tool_call, output, self.available_tools.as_slice());

                return Box::pin(
                    async move {
                        let current_diff = accept_non_zero_exit(
                            agent.context()
                                .exec_cmd(&Command::shell(format!(
                                    "git diff {git_start_sha} --no-color"
                                )))
                                .await,
                        )?
                        .output;

                        let current_diff = if current_diff.is_empty() {
                            None
                        } else {
                            Some(current_diff)
                        };

                        let prompt = prompt.with_context_value("diff", current_diff);

                        let summary = llm.prompt(prompt).await?;
                        tracing::debug!(summary = %summary, original = %output, "Summarized tool output");
                        *output = summary.into();

                        Ok(())
                    }
                    .instrument(span),
                );
            }
            Box::pin(async move { Ok(()) })
        }
    }
}

fn prompt(
    tool: &dyn Tool,
    tool_call: &ToolCall,
    tool_output: &ToolOutput,
    available_tools: &[Box<dyn Tool>],
) -> Prompt {
    // TODO: Partial?
    let formatted_tools = available_tools
        .iter()
        .map(|tool| {
            format!(
                "- **{}**: {}",
                tool.name(),
                // Some tools have large descriptions, only take the first line
                tool.tool_spec()
                    .description
                    .lines()
                    .take(1)
                    .collect::<String>()
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // TODO: clean me up further
    let template =
        Templates::load("tool_summarizer.md").expect("Infallible; failed to find template");

    template
        .with_context_value("formatted_tools", formatted_tools)
        .with_context_value("tool_name", tool.name())
        .with_context_value("tool_description", tool.tool_spec().description)
        .with_context_value("tool_args", tool_call.args())
        .with_context_value("tool_output", tool_output.content())
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use crate::agent::tools::search_file;

    use super::*;

    #[tokio::test]
    async fn test_prompt_rendering() {
        let tool = search_file();

        let tool_call = ToolCall::builder()
            .name("search_file")
            .id("1")
            .args(json!({ "query": "some_file"}).to_string())
            .build()
            .unwrap();
        let tool_output = ToolOutput::Text("Found it!".into());

        let available_tools = vec![Box::new(tool) as Box<dyn Tool>];
        let diff = indoc::indoc!(
            "
            diff --git a/some_file b/some_file
            index 0000000..1111111 100644
            --- a/some_file
            +++ b/some_file
            @@ -1,1 +1,1 @@
            -old
            +new
                "
        );

        let rendered_prompt = prompt(
            &available_tools[0],
            &tool_call,
            &tool_output,
            &available_tools,
        )
        .with_context_value("diff", diff);

        insta::assert_snapshot!(rendered_prompt.render().unwrap());
    }

    #[tokio::test]
    async fn test_prompt_render_where_tool_output_has_jinja() {
        // Create a mock tool
        let tool = search_file();

        // Create a ToolCall for this tool
        let tool_call = ToolCall::builder()
            .name("search_file")
            .id("2")
            .args(json!({ "query": "some_file_with_jinja"}).to_string())
            .build()
            .unwrap();

        // Tool output contains Jinja syntax
        let tool_output = ToolOutput::Text("Result with Jinja syntax: {{ some_variable }}".into());

        // Setup available tools
        let available_tools = vec![Box::new(tool) as Box<dyn Tool>];

        // Generate the prompt
        let rendered_prompt = prompt(
            &available_tools[0],
            &tool_call,
            &tool_output,
            &available_tools,
        )
        .with_context_value("diff", None::<String>);

        // Verify the output to ensure Jinja is escaped
        insta::assert_snapshot!(rendered_prompt.render().unwrap());
    }
}
