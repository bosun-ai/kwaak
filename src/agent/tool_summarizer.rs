use std::sync::Arc;

use swiftide::{
    agents::hooks::AfterToolFn,
    chat_completion::{Tool, ToolCall, ToolOutput},
    prompt::Prompt,
    traits::Command,
    traits::SimplePrompt,
};
use tracing::Instrument as _;

use crate::{git, util::accept_non_zero_exit};

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
    pub fn summarize_hook<'b>(self) -> impl AfterToolFn + 'b
    where
        'a: 'b,
    {
        move |context, tool_call, tool_output| {
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

                let span = tracing::info_span!("summarize_tool", tool = tool.name());
                let prompt = prompt(tool, tool_call, output, self.available_tools.as_slice());

                return Box::pin(
                    async move {
                        let current_diff = accept_non_zero_exit(
                            context
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

    // tfw changing to jinja halfway through
    // The older prompts (like these) are a mess. It would be so nice to have everything in
    // templates and use partials.
}
fn prompt(
    tool: &dyn Tool,
    tool_call: &ToolCall,
    tool_output: &ToolOutput,
    available_tools: &[Box<dyn Tool>],
) -> Prompt {
    let formatted_tools = available_tools
        .iter()
        .map(|tool| format!("- **{}**: {}", tool.name(), tool.tool_spec().description))
        .collect::<Vec<String>>()
        .join("\n");

    let additional_instructions = if tool_call.name() == "run_tests"
        && available_tools
            .iter()
            .find(|t| t.name() == "run_coverage")
            .is_some()
    {
        indoc::formatdoc! {"
                ## Additional instructions
                * If the tests pass, additionally mention that coverage must be checked such that
                  it actually improved, did not stay the same, and the file executed properly.
            "}
    } else {
        String::new()
    };

    indoc::formatdoc!(
            "
            # Goal
            A coding agent has made a tool call. It is your job to refine the output.

            Reformat the following tool output such that it is effective for a chatgpt agent to work with. Only include the reformatted output in your response.
            Reformat but do not summarize, all information should be preserved and detailed.
    
            ## 
            Tool name: {tool_name}
            Tool description: {tool_description}
            Tool was called with arguments: {tool_args}

            {additional_instructions}
    
            {{% if diff -%}}
            ## The agent has made the following changes
            ````
            {{{{ diff }}}}
            ````
            {{% endif %}}

            ## Tool output
            ```
            {tool_output}
            ```
    
            ## Format
            * Only include the reformatted output in your response and nothing else.
            * Include clear instructions on how to fix each issue using the tools that are
              available only.
            * If you do not have a clear solution, state that you do not have a clear solution.
            * If there is any mangling in the tool response, reformat it to be readable.
            {{% if diff -%}}
            * If you suspect that any changes made by the agent affect the tool output, mention
                that. Make sure you include full paths to the files.
            {{% endif -%}}
    
            ## Available tools
            {formatted_tools}

            ## Requirements
            * Only propose improvements that can be fixed by the tools and functions that are
                available in the conversation. For instance, running a command to fix linting can also be fixed by writing to that file without errors.
            * If the tool output has repeating patterns, only include the pattern once and state
             that it happens multiple times.
            ", tool_name = tool.name(), tool_description = tool.tool_spec().description, tool_args = tool_call.args().unwrap_or_default(), tool_output = tool_output.content().unwrap_or_default().replace('{', "\\{")).into()
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

        insta::assert_snapshot!(rendered_prompt.render().await.unwrap());
    }
}
