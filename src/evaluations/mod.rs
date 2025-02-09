use anyhow::Result;

mod logging_responder;
mod output;
pub mod patch;
mod tool_evaluation_agent;

#[cfg(test)]
mod tests;

pub use tool_evaluation_agent::start_tool_evaluation_agent;

pub async fn run_patch_evaluation(iterations: u32) -> Result<()> {
    println!("Running patch evaluation with {iterations} iterations");
    patch::evaluate(iterations).await
}
