use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

pub const DIMS: usize = 384;

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn load() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
            .join("memso")
            .join("models");

        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::BGESmallENV15)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(true),
        )
        .context("Failed to load embedding model")?;

        Ok(Self { model })
    }

    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let mut results = self
            .model
            .embed(vec![text], None)
            .context("Embedding failed")?;
        Ok(results.remove(0))
    }
}
