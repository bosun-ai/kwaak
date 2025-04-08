//! This module implements a conversation summarizer
//!
//! The summarizer counts number of completions in an agent, and if the count surpasses a set
//! value, the conversation is summarized and a summary is added to the conversation.
//!
//! Often when there are many messages, the context window can get too large for the agent to
//! effectively complete. This summarizer helps to keep the context window small and steers the
//! agent towards a more focused solution.
//!
//! Because it also acts as a nice opportunity to steer, we will also include a steering prompt
//! and the current diff
//!
//! The agent completes messages since the last summary.
use std::sync::{atomic::AtomicUsize, Arc};

use swiftide::{
    agents::hooks::AfterEachFn,
    chat_completion::{ChatCompletion, ChatMessage, Tool},
    prompt::Prompt,
    traits::Command,
};
use tracing::Instrument as _;

use crate::util::accept_non_zero_exit;

#[derive(Clone)]
pub struct ConversationSummarizer {
    llm: Arc<dyn ChatCompletion>,
    available_tools: Vec<Box<dyn Tool>>,
    num_completions_since_summary: Arc<AtomicUsize>,
    git_start_sha: String,
    num_completions_for_summary: usize,
    initial_query: String,
}

impl ConversationSummarizer {
    pub fn new(
        llm: Box<dyn ChatCompletion>,
        available_tools: &[Box<dyn Tool>],
        git_start_sha: impl Into<String>,
        num_completions_for_summary: usize,
        initial_query: impl Into<String>,
    ) -> Self {
        Self {
            llm: llm.into(),
            available_tools: available_tools.into(),
            num_completions_since_summary: Arc::new(0.into()),
            git_start_sha: git_start_sha.into(),
            num_completions_for_summary,
            initial_query: initial_query.into(),
        }
    }

    #[must_use]
    pub fn summarize_hook(self) -> impl AfterEachFn {
        move |agent| {
            let llm = self.llm.clone();

            let span = tracing::info_span!("summarize_conversation");

            let current_count = self
                .num_completions_since_summary
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            if current_count < self.num_completions_for_summary || agent.is_stopped() {
                tracing::debug!(current_count, "Not enough completions for summary");

                return Box::pin(async move { Ok(()) });
            }

            self.num_completions_since_summary
                .store(0, std::sync::atomic::Ordering::SeqCst);

            let prompt = self.prompt();
            let git_start_sha = self.git_start_sha.clone();
            let initial_query = self.initial_query.clone();

            Box::pin(
                async move {
                    let current_diff = accept_non_zero_exit(
                        agent
                            .context()
                            .exec_cmd(&Command::shell(format!(
                                "git diff {git_start_sha} --no-color"
                            )))
                            .await,
                    )?
                    .output;

                    let prompt = prompt.with_context_value("diff", current_diff).render()?;

                    let mut messages =
                        filter_messages_since_summary(agent.context().history().await);
                    messages.push(ChatMessage::new_user(prompt));

                    let mut summary = llm.complete(&messages.into()).await?;

                    // reinject the initial query at the beginning of the summary
                    //
                    // NOTE use code block (```) for the original goal as it may contain markdown
                    // itself which should be distinguished from the markdown in the rest of the
                    // summary
                    let msg = summary.message.unwrap_or_default();
                    let msg = format!(
                        "# Original Goal for Reference: \n```\n{initial_query}\n```\n\n{msg}"
                    );
                    summary.message = Some(msg);

                    if let Some(summary) = summary.message() {
                        tracing::debug!(summary = %summary, "Summarized conversation");
                        agent
                            .context()
                            .add_message(ChatMessage::new_summary(summary))
                            .await;
                    } else {
                        tracing::error!("No summary generated, this is a bug");
                    }

                    Ok(())
                }
                .instrument(span),
            )
        }
    }

    // tfw changing to jinja halfway through
    fn prompt(&self) -> Prompt {
        let available_tools = self
            .available_tools
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

        indoc::formatdoc!(
            "
        # Goal
        Summarize and review the conversation up to this point. An agent has tried to achieve a goal, and you need to help them get there.
            
        ## Requirements
        * Only include the summary in your response and nothing else.
        * If any, mention every file changed
        * When mentioning files include the full path
        * Be very precise and critical
        * If the agent was reading files in order to understand a solution, provide a detailed
            summary of any specific relevant code which will be useful for the agents next steps
            such that the agent can work directly from the summarized code without having to reread
            the files. Do not summarize code or files which are not directly relevant to the agents
            next steps. When summarizing code include the exact names of objects and functions, as
            well as detailed explanations of what they are used for and how they work. Also provide
            the snippets of the actual code which is relevant to the agents next steps such that
            the agent will not have to reread the files. Do not include entire files, only relevant
            snippets of code.
        * If a previous solution did not work, include that in your response. If a reason was
            given, include that as well.
        * Include any previous summaries in your response
        * Include every step so far taken and clearly state where the agent is at,
            especially in relation to the initial goal. Include any observations or information
            made, and include your own where relevant to achieving the goal.
        * Be extra detailed on the last steps taken
        * Provide clear instructions on how to proceed. If applicable, include the tools that
            should be used.
        * Identify the bigger goal the user wanted to achieve and clearly restate it
        * If the goal is not yet achieved, reflect on why and provide a clear path forward
        * In suggested next steps, talk in the future tense. For example: \"You should run the tests\"
        * Do not provide the actual tool calls the agent should still make, provide the steps and
            necessary context instead. Assume the agent knows how to use the tools.

        {{% if diff -%}}
        ## Current changes made
        ````
        {{{{ diff }}}}
        ````
        {{% endif %}}

        ## Available tools
        {available_tools}

        ## Format
        * Start your response with the following header '# Summary'
        * Phrase each bullet point as if talking about 'you'
        
        # Example format
        ```
        # Summary

        ## Your goal
        <Your goal>

        ## Since then you did
        * <summary of each step>
        * You tried to run the tests but they failed. Here is why <...>
        * You read a file called `full/path/to/file.txt` and here is what you learned <...>

        ## Relevant files
        <Summary of relevant code which was read including the exact names of objects and functions
         also include snippets of the actual code which is relevant to the agents next steps such that the agent will not have
         to reread the files. Do not include entire files, only relevant snippets of code.>  

        ## Reflection
        <Reflection on the steps you took and why you took them. What have you observed and what have you learned so far>
        
        ## Suggested next steps
        1. <Suggested step>
        ```
        ",
            available_tools = available_tools
        )
        .into()
    }
}

fn filter_messages_since_summary(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    // Filter out all messages up to and including the last summary
    let mut summary_found = false;
    let mut messages = messages
        .into_iter()
        .rev()
        .filter_map(|m| {
            // If we have found a summary, we are good
            if summary_found {
                return None;
            }

            // Ignore the system prompt, it only misdirects
            if matches!(m, ChatMessage::System(..)) {
                return None;
            }

            // Tool outputs are formatted as assistant messages
            if let ChatMessage::ToolOutput(tool_call,tool_output) = &m {
                let message = format!("I ran a tool called: {} with the following arguments: {}\n The tool returned:\n{}", tool_call.name(), tool_call.args().unwrap_or("No arguments"), tool_output.content().unwrap_or("No output"));
                return Some(ChatMessage::Assistant(Some(message), None));
            }

            // For assistant messages, we only keep those with messages in them
            if let ChatMessage::Assistant(message, Some(..)) = &m {
                if message.is_some() {
                    return Some(ChatMessage::Assistant(message.clone(), None));
                }
                return None;
            }
            if let ChatMessage::Summary(_) = m {
                summary_found = true;
            }
            Some(m)
        })
        .collect::<Vec<_>>();

    messages.reverse();

    messages
}

#[cfg(test)]
mod tests {
    use super::*;
    use swiftide::chat_completion::{ChatMessage, ToolCallBuilder};

    #[test]
    fn test_filter_messages_since_summary_no_summary() {
        let messages = vec![
            ChatMessage::new_user("User message 1"),
            ChatMessage::new_assistant(Some("Assistant message 1"), None),
            ChatMessage::new_user("User message 2"),
        ];

        let filtered_messages = filter_messages_since_summary(messages.clone());
        assert_eq!(filtered_messages, messages);
    }

    #[test]
    fn test_filter_messages_since_summary_with_summary() {
        let messages = vec![
            ChatMessage::new_system("System message 1"),
            ChatMessage::new_user("User message 1"),
            ChatMessage::new_assistant(Some("Assistant message 1"), None),
            ChatMessage::new_summary("Summary message"),
            ChatMessage::new_user("User message 2"),
            ChatMessage::new_assistant(Some("Assistant message 2"), None),
        ];

        let filtered_messages = filter_messages_since_summary(messages);
        assert_eq!(
            filtered_messages,
            vec![
                ChatMessage::new_summary("Summary message"),
                ChatMessage::new_user("User message 2"),
                ChatMessage::new_assistant(Some("Assistant message 2"), None),
            ]
        );
    }

    #[test]
    fn test_filter_messages_since_summary_with_tool_output() {
        let tool_call = ToolCallBuilder::default()
            .name("run_tests")
            .id("1")
            .build()
            .unwrap();
        let messages = vec![
            ChatMessage::new_user("User message 1"),
            ChatMessage::new_assistant(Some("Assistant message 1"), None),
            ChatMessage::new_summary("Summary message"),
            ChatMessage::new_user("User message 2"),
            ChatMessage::new_assistant(Some("Assistant message 2"), Some(vec![tool_call.clone()])),
            ChatMessage::new_tool_output(tool_call, "Tool output message"),
        ];

        let filtered_messages = filter_messages_since_summary(messages);
        assert_eq!(
            filtered_messages,
            vec![
                ChatMessage::new_summary("Summary message"),
                ChatMessage::new_user("User message 2"),
                ChatMessage::new_assistant(Some("Assistant message 2"), None),
                ChatMessage::new_assistant(
                    Some(
                        "I ran a tool called: run_tests with the following arguments: No arguments\n The tool returned:\nTool output message"
                    ),
                    None
                )
            ]
        );
    }

    #[test]
    fn test_filter_messages_since_summary_multiple_summaries() {
        let messages = vec![
            ChatMessage::new_user("User message 1"),
            ChatMessage::new_summary("Summary message 1"),
            ChatMessage::new_user("User message 2"),
            ChatMessage::new_summary("Summary message 2"),
            ChatMessage::new_user("User message 3"),
            ChatMessage::new_assistant(Some("Assistant message 3"), None),
        ];

        let filtered_messages = filter_messages_since_summary(messages);
        assert_eq!(
            filtered_messages,
            vec![
                ChatMessage::new_summary("Summary message 2"),
                ChatMessage::new_user("User message 3"),
                ChatMessage::new_assistant(Some("Assistant message 3"), None),
            ]
        );
    }

    #[test]
    fn test_filters_assistant_messages_with_only_tool_outputs() {
        let tool_call = ToolCallBuilder::default()
            .name("run_tests")
            .id("1")
            .build()
            .unwrap();
        let messages = vec![
            ChatMessage::new_user("User message 1"),
            ChatMessage::new_assistant(Some("Assistant message 1"), None),
            ChatMessage::new_summary("Summary message"),
            ChatMessage::new_user("User message 2"),
            ChatMessage::new_assistant(None::<String>, Some(vec![tool_call.clone()])),
            ChatMessage::new_tool_output(tool_call, "Tool output message"),
        ];

        let filtered_messages = filter_messages_since_summary(messages);
        assert_eq!(
            filtered_messages,
            vec![
                ChatMessage::new_summary("Summary message"),
                ChatMessage::new_user("User message 2"),
                ChatMessage::new_assistant(
                    Some(
                        "I ran a tool called: run_tests with the following arguments: No arguments\n The tool returned:\nTool output message"
                    ),
                    None
                )
            ]
        );
    }
}
