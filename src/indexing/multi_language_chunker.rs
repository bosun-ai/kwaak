use anyhow::Result;
use std::collections::HashMap;
use swiftide::traits::ChunkerTransformer;

use swiftide::{
    indexing::{
        transformers::{self, ChunkCode},
        IndexingStream, Node,
    },
    integrations::treesitter::{ChunkSize, SupportedLanguages},
};

#[derive(Debug, Clone)]
pub struct MultiLanguageChunker {
    chunkers: HashMap<Vec<String>, transformers::ChunkCode>,
}

impl MultiLanguageChunker {
    pub fn try_for_languages_and_chunk_size(
        languages: &[SupportedLanguages],
        chunk_size: impl Into<ChunkSize>,
    ) -> Result<Self> {
        let mut chunkers = HashMap::new();
        let chunk_size = chunk_size.into();

        for lang in languages {
            let chunker = ChunkCode::try_for_language_and_chunk_size(*lang, chunk_size.clone())?;
            let extensions = lang
                .file_extensions()
                .iter()
                .map(ToString::to_string)
                .collect();
            chunkers.insert(extensions, chunker);
        }

        Ok(Self { chunkers })
    }

    fn find_chunker(&self, node: &Node) -> Option<&transformers::ChunkCode> {
        let node_extensions = node.path.extension()?.to_string_lossy().to_string();

        self.chunkers.iter().find_map(|(extensions, chunker)| {
            if extensions.contains(&node_extensions) {
                Some(chunker)
            } else {
                None
            }
        })
    }
}

#[async_trait::async_trait]
impl ChunkerTransformer for MultiLanguageChunker {
    async fn transform_node(&self, node: swiftide::indexing::Node) -> IndexingStream {
        if let Some(chunker) = self.find_chunker(&node) {
            chunker.transform_node(node).await
        } else {
            // If no chunker is available for the language, return the node as is
            anyhow::anyhow!(
                "Extension not supported for chunking: {}",
                node.path.display()
            )
            .into()
        }
    }
}
