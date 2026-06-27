use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, ModelTrait, TextEmbedding};

use crate::config::env_var_or_unset;

pub const DIMS: usize = 384;

const MODEL: EmbeddingModel = EmbeddingModel::BGESmallENV15;

/// The HuggingFace model code for the active embedding model.
/// If DIMS is updated, write a schema migration to resize F32_BLOB accordingly.
pub fn model_id() -> String {
    EmbeddingModel::get_model_info(&MODEL)
        .map(|info| info.model_code.clone())
        .unwrap_or_else(|| MODEL.to_string())
}

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn load() -> Result<Self> {
        let cache_dir = if let Some(dir) = env_var_or_unset("COREE_MODEL_DIR") {
            std::path::PathBuf::from(dir)
        } else {
            let dir = dirs::cache_dir()
                .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
                .join("coree")
                .join("models");
            if !dir.exists() {
                eprintln!(
                    "[coree] Downloading embedding model on first run. This may take a moment..."
                );
            }
            dir
        };

        // COREE_FORCE_MODEL_REFRESH=1: delete the model cache before loading so
        // fastembed re-downloads a fresh copy. Useful for troubleshooting a
        // corrupted model or testing the cold-start download path locally.
        if env_var_or_unset("COREE_FORCE_MODEL_REFRESH").as_deref() == Some("1") && cache_dir.exists() {
            std::fs::remove_dir_all(&cache_dir)
                .context("COREE_FORCE_MODEL_REFRESH: failed to remove model cache")?;
        }

        // Serialize first-time model downloads across every coree process on this
        // machine. The model cache is global (shared by all projects), but each
        // project's `serve` elects its own primary and loads the embedder
        // independently, so two cold-start serves in different projects would race
        // on hf-hub's per-blob download lock -- which is non-blocking with only a
        // ~5s retry budget (hf-hub-0.5.0 api/sync.rs) -- and the loser fails with
        // "Lock acquisition failed". A machine-global blocking advisory lock makes
        // the loser wait for the winner's download, then load from the warm cache.
        //
        // Held only for the duration of this load() call (released when the guard
        // drops). Best-effort: if the lock can't be taken (e.g. read-only FS) we
        // proceed unlocked rather than failing the load.
        let _download_guard = acquire_download_lock(&cache_dir);

        let model = TextEmbedding::try_new(
            InitOptions::new(MODEL)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(true),
        )
        .context("Failed to load embedding model")?;

        Ok(Self { model })
    }

    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let t = std::time::Instant::now();
        let results = self
            .model
            .embed(vec![text], None)
            .context("Embedding failed")?;
        tracing::debug!(
            elapsed_ms = t.elapsed().as_millis(),
            chars = text.len(),
            "embed"
        );
        results
            .into_iter()
            .next()
            .context("Embedding model returned no results")
    }
}

/// Acquire a machine-global advisory lock that serializes model downloads across
/// coree processes. Returns the locked file handle as a guard; the lock is released
/// when it is dropped.
///
/// Crash-safe on all platforms: this is an OS advisory lock bound to the open file
/// handle (`File::lock` -> `flock` on unix, `LockFileEx` on windows), so the kernel
/// releases it automatically on any process exit, including a crash. The on-disk
/// `.download.lock` file is therefore inert if left behind and never needs manual
/// cleanup -- the same primitive coree already uses for `serve.lock`.
///
/// Best-effort: any failure to create the dir, open the file, or take the lock
/// returns `None`, leaving the caller to proceed without coordination.
fn acquire_download_lock(cache_dir: &std::path::Path) -> Option<std::fs::File> {
    std::fs::create_dir_all(cache_dir).ok()?;
    let lock_path = cache_dir.join(".download.lock");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .ok()?;
    // Blocking exclusive lock: a concurrent cold-start waits here until the process
    // currently downloading finishes, then proceeds against the now-warm cache.
    file.lock().ok()?;
    Some(file)
}

/// Encode a float slice as a little-endian byte blob for libsql vector storage.
/// Shared by store and retrieve to avoid duplication.
pub fn floats_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Decode a little-endian byte blob back to a float slice. Inverse of floats_to_blob.
pub fn blob_to_floats(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .filter_map(|c| c.try_into().ok())
        .map(f32::from_le_bytes)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_to_blob_roundtrip() {
        let floats = vec![1.0f32, 2.0f32, -3.5f32];
        let blob = floats_to_blob(&floats);
        assert_eq!(blob.len(), 12); // 3 floats * 4 bytes each
        let decoded: Vec<f32> = blob
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        assert_eq!(decoded, floats);
    }
}
