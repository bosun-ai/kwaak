//! Evaluate with RAGAS
use anyhow::Result;
use std::path::Path;

use swiftide::query::evaluators::ragas;

use crate::{indexing::build_query_pipeline, repository::Repository};

/// Evaluate a query pipeline with RAGAS
///
/// Ragas is a simple model with 4 base metrics to evaluate rag pipelines.
///
/// Takes an input file as a base dataset and will write to the output file as json.
///
/// Example format of the files:
///
/// ```json
/// {
///   "What is the capital of France?": {
///     "question": "What is the capital of France?",
///
///     // The answer given by the pipeline
///     "answer": "Paris",
///
///     // Retrieved documents
///     "contexts": ["The capital of France is Paris"],
///
///     // The expected answer
///     "ground_truth": "Paris",
///     }
/// }
/// ```
///
///
/// See [ragas](https://ragas.io) for more information.
pub async fn evaluate_query_pipeline(
    repository: &Repository,
    input: &Path,
    output: &Path,
    record_ground_truth: bool,
) -> Result<()> {
    // Load dataset
    tracing::info!("Loading dataset from {}", input.display());
    let dataset: ragas::EvaluationDataSet = std::fs::read_to_string(input)?.parse()?;

    // Setup ragas
    tracing::info!("Setting up evaluator");
    let ragas = ragas::Ragas::from_prepared_questions(dataset);

    // Build query pipeline
    tracing::info!("Building query pipeline");
    let pipeline = build_query_pipeline(repository, Some(Box::new(ragas.clone())))?;

    // Query all questions from the dataset
    tracing::info!("Querying all questions");
    pipeline.query_all(ragas.questions().await).await?;

    if record_ground_truth {
        // Record ground truth
        tracing::info!("Recording ground truth");
        ragas.record_answers_as_ground_truth().await;
    }

    tracing::info!("Writing evaluation results to file");
    // Export to file
    let json = ragas.to_json().await;
    std::fs::write(output, json)?;

    Ok(())
}
