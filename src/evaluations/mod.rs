use anyhow::Result;

mod evaluation_agent;
mod logging_responder;
mod output;
pub mod patch;

pub use evaluation_agent::{get_evaluation_tools, start_evaluation_agent};

pub async fn run_patch_evaluation(iterations: u32) -> Result<()> {
    println!("Running patch evaluation with {} iterations", iterations);
    patch::evaluate(iterations).await
}
