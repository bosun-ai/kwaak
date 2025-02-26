use anyhow::Result;
use indoc::formatdoc;
use swiftide::{
    query::{
        self, answers, query_transformers, search_strategies::SimilaritySingleEmbedding, states,
        Query,
    },
    traits::{EmbeddingModel, SimplePrompt},
};

use crate::{repository::Repository, storage, templates::Templates, util::strip_markdown_tags};

#[tracing::instrument(skip_all, err)]
pub async fn query(repository: &Repository, query: impl AsRef<str>) -> Result<String> {
    let answer = build_query_pipeline(repository)?
        .query(query.as_ref())
        .await?
        .answer()
        .to_string();
    Ok(strip_markdown_tags(&answer))
}

/// Builds a query pipeline
///
/// # Panics
///
/// Should be infallible
pub fn build_query_pipeline<'b>(
    repository: &Repository,
) -> Result<query::Pipeline<'b, SimilaritySingleEmbedding, states::Answered>> {
    let backoff = repository.config().backoff;
    let query_provider: Box<dyn SimplePrompt> = repository
        .config()
        .query_provider()
        .get_simple_prompt_model(backoff)?;
    let embedding_provider: Box<dyn EmbeddingModel> = repository
        .config()
        .embedding_provider()
        .get_embedding_model(backoff)?;

    let lancedb = storage::get_lancedb(repository);
    let search_strategy: SimilaritySingleEmbedding<()> = SimilaritySingleEmbedding::default()
        .with_top_k(30)
        .to_owned();

    let prompt_template = Templates::from_file("agentic_answer_prompt.md")?;
    let document_template = Templates::from_file("indexing_document.md")?;

    // NOTE: Changed a lot to tailor it for agentic flows, might be worth upstreaming
    let simple = answers::Simple::builder()
        .client(query_provider.clone())
        .prompt_template(prompt_template.into())
        .document_template(document_template)
        .build()
        .expect("infallible");

    let language = repository.config().language.to_string();
    let project = repository.config().project_name.clone();

    Ok(query::Pipeline::from_search_strategy(search_strategy)
        .then_transform_query(move |mut query: Query<states::Pending>| {
            let current = query.current();
            query.transformed_query(formatdoc! {"
                {current}

                The project is written in: {language}
                The project is called: {project}

            "});

            Ok(query)
        })
        .then_transform_query(query_transformers::GenerateSubquestions::from_client(
            query_provider.clone(),
        ))
        .then_transform_query(query_transformers::Embed::from_client(
            embedding_provider.clone(),
        ))
        .then_retrieve(lancedb)
        // .then_transform_response(response_transformers::Summary::from_client(
        //     query_provider.clone(),
        // ))
        .then_answer(simple))
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_rendering_document() {
        use insta::assert_snapshot;
        use swiftide::{indexing::Metadata, query::Document};
        use tera::Context;

        use crate::templates::Templates;

        let document = Document::new(
            "This is a test document",
            Some(Metadata::from([
                ("path", serde_json::Value::from("my file")),
                ("soups", serde_json::Value::from(["tomato", "snert"])),
                ("empty", serde_json::Value::from("")),
            ])),
        );

        assert_snapshot!(Templates::render(
            "indexing_document.md",
            &Context::from_serialize(document).unwrap(),
        )
        .unwrap());
    }
}
