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
}

impl ConversationSummarizer {
    pub fn new(
        llm: Box<dyn ChatCompletion>,
        available_tools: &[Box<dyn Tool>],
        git_start_sha: impl Into<String>,
        num_completions_for_summary: usize,
    ) -> Self {
        Self {
            llm: llm.into(),
            available_tools: available_tools.into(),
            num_completions_since_summary: Arc::new(0.into()),
            git_start_sha: git_start_sha.into(),
            num_completions_for_summary,
        }
    }

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

                    let prompt = prompt
                        .with_context_value("diff", current_diff)
                        .render()
                        .await?;

                    let mut messages =
                        filter_messages_since_summary(agent.context().history().await);
                    messages.push(ChatMessage::new_user(prompt));

                    let summary = llm.complete(&messages.into()).await?;

                    if let Some(summary) = summary.message() {
                        tracing::debug!(summary = %summary, "Summarized tool output");
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
        Summarize and review the conversation up to this point
            
        ## Requirements
        * Only include the summary in your response and nothing else.
        * If any, mention every file changed
        * When mentioning files include the full path
        * Be very precise and critical
        * If a previous solution did not work, include that in your response. If a reason was
            given, include that as well.
        * Include any previous summaries in your response
        * Include every step so far taken concisely and clearly state where the agent is at,
            especially in relation to the initial goal.
        * Be extra detailed on the last step taken
        * Provide clear instructions on how to proceed. If applicable, include the tools that
            should be used.
        * Identify the bigger goal the user wanted to achieve and clearly restate it
        * If the goal is not yet achieved, reflect on why and provide a clear path forward

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

        ## Previously you did
        * <concise summary of each step>
        * You tried to run the tests but they failed. Here is why <...>

        ## Since then you did
        * <Summary of steps since the last summary>

        ## Reflection
        <Concise reflection on the steps you took and why you took them>
        
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
            if summary_found {
                return None;
            }
            if m.is_tool_output() {
                return None;
            }
            if let ChatMessage::Assistant(message, Some(..)) = &m {
                if message.is_some() {
                    return Some(ChatMessage::Assistant(message.clone(), None));
                }
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
            ChatMessage::new_tool_output(tool_call, "Tool output me_ssage"),
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
}
