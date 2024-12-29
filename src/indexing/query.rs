use anyhow::Result;
use swiftide::{
    query::{
        self, answers, query_transformers, search_strategies::SimilaritySingleEmbedding, states,
    },
    traits::{EmbeddingModel, SimplePrompt},
};

use crate::{repository::Repository, storage, util::strip_markdown_tags};

#[tracing::instrument(skip_all, err)]
pub async fn query(repository: &Repository, query: impl AsRef<str>) -> Result<String> {
    let answer = build_query_pipeline(repository)?
        .query(query.as_ref())
        .await?
        .answer()
        .to_string();
    Ok(strip_markdown_tags(&answer))
}

pub fn build_query_pipeline<'b>(
    repository: &Repository,
) -> Result<query::Pipeline<'b, SimilaritySingleEmbedding, states::Answered>> {
    let query_provider: Box<dyn SimplePrompt> = repository.config().query_provider().try_into()?;
    let embedding_provider: Box<dyn EmbeddingModel> =
        repository.config().embedding_provider().try_into()?;

    let lancedb = storage::build_lancedb(repository)?;
    let search_strategy: SimilaritySingleEmbedding<()> = SimilaritySingleEmbedding::default()
        .with_top_k(20)
        .to_owned();

    Ok(query::Pipeline::from_search_strategy(search_strategy)
        .then_transform_query(query_transformers::GenerateSubquestions::from_client(
            query_provider.clone(),
        ))
        .then_transform_query(query_transformers::Embed::from_client(
            embedding_provider.clone(),
        ))
        .then_retrieve(lancedb.build()?)
        // .then_transform_response(response_transformers::Summary::from_client(
        //     query_provider.clone(),
        // ))
        .then_answer(answers::Simple::from_client(query_provider.clone())))
}
