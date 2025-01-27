use anyhow::Result;

pub mod patch;

pub async fn run_patch_evaluation(iterations: u32) -> Result<()> {
    println!("Running patch evaluation with {} iterations", iterations);
    patch::evaluate(iterations).await
}
