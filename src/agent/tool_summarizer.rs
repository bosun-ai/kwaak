use std::sync::Arc;

use swiftide::{
    agents::hooks::AfterToolFn,
    chat_completion::{Tool, ToolCall, ToolOutput},
    prompt::Prompt,
    traits::SimplePrompt,
};
use tracing::Instrument as _;

#[derive(Clone)]
pub struct ToolSummarizer<'a> {
    llm: Arc<dyn SimplePrompt>,
    tools_to_summarize: Vec<&'a str>,
    available_tools: Vec<Box<dyn Tool>>,
}

impl<'a> ToolSummarizer<'a> {
    pub fn new(
        llm: Box<dyn SimplePrompt>,
        tools_to_summarize: &[&'a str],
        available_tools: &[Box<dyn Tool>],
    ) -> Self {
        Self {
            llm: llm.into(),
            tools_to_summarize: tools_to_summarize.into(),
            available_tools: available_tools.into(),
        }
    }

    // Rust has a long outstanding issue where it captures outer lifetimes when returning an impl
    // that also has lifetimes, see https://github.com/rust-lang/rust/issues/42940
    pub fn summarize_hook<'b>(self) -> impl AfterToolFn + 'b
    where
        'a: 'b,
    {
        move |_context, tool_call, tool_output| {
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
                let prompt = self.prompt(tool, tool_call, output);

                let span = tracing::info_span!("summarize_tool", tool = tool.name());
                return Box::pin(
                    async move {
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

    fn prompt(&self, tool: &dyn Tool, tool_call: &ToolCall, tool_output: &ToolOutput) -> Prompt {
        let available_tools = self
            .available_tools
            .iter()
            .map(|tool| format!("- **{}**: {}", tool.name(), tool.tool_spec().description))
            .collect::<Vec<String>>()
            .join("\n");

        indoc::formatdoc!(
            "
            # Goal
            Reformat the following tool output such that it is effective for a chatgpt agent to work with. Only include the reformatted output in your response.
            Reformat but do not summarize, all information should be preserved and detailed.
    
            ## Additional Context
            Tool name: {tool_name}
            Tool description: {tool_description}
            Tool was called with arguments: {tool_args}
    
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
    
            ## Available tools
            {available_tools}

            ## Requirements
            * Only propose improvements that can be fixed by the tools and functions that are
                availalbe in the conversation. For instance, running a command to fix linting can also be fixed by writing to that file without errors.
            * If the tool output has repeating patterns, only include the pattern once and state
             that it happens multiple times.
            ", tool_name = tool.name(), tool_description = tool.tool_spec().description, tool_args = tool_call.args().unwrap_or_default(), tool_output = tool_output.content().unwrap_or_default()).into()
    }
}
