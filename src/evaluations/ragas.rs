//! Evaluate with RAGAS
use anyhow::Result;
use std::path::Path;

use swiftide::query::evaluators::ragas;

use crate::{indexing::build_query_pipeline, repository::Repository, storage};

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
    input: Option<&Path>,
    output: &Path,
    questions: Option<&[String]>,
    record_ground_truth: bool,
) -> Result<()> {
    // Load dataset

    let dataset: ragas::EvaluationDataSet = if let Some(input) = input {
        println!("Reading dataset from file: {}", input.display());
        std::fs::read_to_string(input)?.parse()?
    } else if let Some(questions) = questions {
        println!("Generation evaluations for {} questions", questions.len());
        questions.into()
    } else {
        anyhow::bail!("Either input or questions must be provided")
    };

    // Setup ragas
    tracing::info!("Setting up evaluator");
    let ragas = ragas::Ragas::from_prepared_questions(dataset);

    // Build query pipeline
    tracing::info!("Building query pipeline");
    let pipeline = build_query_pipeline(
        repository,
        &storage::get_duckdb(repository),
        Some(Box::new(ragas.clone())),
    )?;

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
    if let Some(parent) = output.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(output, json)?;

    Ok(())
}
