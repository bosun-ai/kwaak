use anyhow::Result;

pub mod patch;
mod evaluation_agent;

pub use evaluation_agent::{get_evaluation_tools, start_evaluation_agent};

pub async fn run_patch_evaluation(iterations: u32) -> Result<()> {
    println!("Running patch evaluation with {} iterations", iterations);
    patch::evaluate(iterations).await
}
