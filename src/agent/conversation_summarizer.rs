//! This module implements a conversation summarizer
//!
//! The summarizer counts number of completions in an agent, and if the count surpasses a set
//! value, the conversation is summarized and a summary is added to the conversation.
//!
//! Often when there are many messages, the context window can get too large for the agent to
//! effectively complete. This summarizer helps to keep the context window small and steers the
//! agent towards a more focused solution.
//!
//! The agent completes messages since the last summary.
use std::sync::{atomic::AtomicUsize, Arc};

use swiftide::{
    agents::hooks::AfterEachFn,
    chat_completion::{ChatCompletion, ChatMessage, Tool},
};
use tracing::Instrument as _;

const NUM_COMPLETIONS_FOR_SUMMARY: usize = 10;

#[derive(Clone)]
pub struct ConversationSummarizer {
    llm: Arc<dyn ChatCompletion>,
    available_tools: Vec<Box<dyn Tool>>,
    num_completions_since_summary: Arc<AtomicUsize>,
}

impl ConversationSummarizer {
    pub fn new(llm: Box<dyn ChatCompletion>, available_tools: &[Box<dyn Tool>]) -> Self {
        Self {
            llm: llm.into(),
            available_tools: available_tools.into(),
            num_completions_since_summary: Arc::new(0.into()),
        }
    }

    pub fn summarize_hook(self) -> impl AfterEachFn {
        move |context| {
            let llm = self.llm.clone();

            let prompt = self.prompt();

            let span = tracing::info_span!("summarize_conversation");

            let current_count = self
                .num_completions_since_summary
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            if current_count < NUM_COMPLETIONS_FOR_SUMMARY {
                tracing::debug!(current_count, "Not enough completions for summary");

                return Box::pin(async move { Ok(()) });
            }

            self.num_completions_since_summary
                .store(0, std::sync::atomic::Ordering::SeqCst);

            Box::pin(
                async move {
                    let mut messages = filter_messages_since_summary(context.history().await);
                    messages.push(ChatMessage::new_user(prompt));

                    let summary = llm.complete(&messages.into()).await?;

                    if let Some(summary) = summary.message() {
                        tracing::debug!(summary = %summary, "Summarized tool output");
                        context.add_message(ChatMessage::new_summary(summary)).await;
                    } else {
                        tracing::error!("No summary generated, this is a bug");
                    }

                    Ok(())
                }
                .instrument(span),
            )
        }
    }

    fn prompt(&self) -> String {
        let available_tools = self
            .available_tools
            .iter()
            .map(|tool| format!("- **{}**: {}", tool.name(), tool.tool_spec().description))
            .collect::<Vec<String>>()
            .join("\n");

        indoc::formatdoc!(
            "
        # Goal
        Summarize the conversation up to this point
            
        ## Requirements
        * Only include the summary in your response and nothing else.
        * When mentioning files include the full path
        * Be very precise
        * If a previous solution did not work, include that in your response. If a reason was
            given, include that as well.
        * Include any previous summaries in your response
        * Be extra detailed on the last step taken
        * Provide clear instructions on how to proceed. If applicable, include the tools that
            should be used.
        * Identify the goal the user wanted to achieve and clearly restate it

        ## Available tools
        {available_tools}

        ## Format
        * Start your response with the following header '# Summary'
        * Phrase each bullet point as if talking about 'you'
        
        # Example format
        ```
        # Summary

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
    }
}

fn filter_messages_since_summary(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut summary_found = false;
    let mut messages = messages
        .into_iter()
        .rev()
        .filter(|m| {
            if summary_found {
                return matches!(m, ChatMessage::System(_));
            }
            if let ChatMessage::Summary(_) = m {
                summary_found = true;
            }
            true
        })
        .collect::<Vec<_>>();

    messages.reverse();

    messages
}
