use anyhow::Result;
use async_trait::async_trait;

use super::git::{self, CommitInfo};
use super::parser;
use super::search::CodeResult;

pub struct IndexedFile {
    pub rel_path: String,
    pub content_hash: String,
    pub language: String,
    pub churn_count: i64,
    pub hotspot_score: f64,
    pub commits: Vec<git::CommitStat>,
    pub chunks: Vec<IndexedChunk>,
}

pub struct IndexedChunk {
    pub chunk: parser::Chunk,
    pub content_hash: String,
    pub embedding: Vec<f32>,
}

#[async_trait]
pub trait CodeIndexStore: Send + Sync {
    async fn stored_logic_version(&self) -> Result<Option<u32>>;

    async fn set_stored_logic_version(&self, version: u32) -> Result<()>;

    async fn clear_all(&self) -> Result<()>;

    async fn file_content_hash(&self, rel_path: &str) -> Result<Option<String>>;

    async fn replace_file(&self, file: IndexedFile) -> Result<()>;

    async fn remove_file(&self, rel_path: &str) -> Result<()>;

    async fn search_code(
        &self,
        embedding: Vec<f32>,
        query: &str,
        limit: usize,
    ) -> Result<Vec<CodeResult>>;

    async fn get_symbol(
        &self,
        name: &str,
        file_path: Option<&str>,
    ) -> Result<Vec<CodeResult>>;

    async fn get_coupled_files(
        &self,
        file_path: &str,
        limit: usize,
    ) -> Result<Vec<(String, i64, String)>>;

    async fn record_commit(
        &self,
        commit: &CommitInfo,
        file_updates: &[(String, i64, f64)],
    ) -> Result<()>;
}

#[cfg(test)]
pub(crate) mod test_suite {
    use super::*;
    use std::sync::Arc;

    pub(crate) fn dummy_chunk(symbol_name: &str, qualified_name: &str) -> IndexedChunk {
        IndexedChunk {
            chunk: parser::Chunk {
                symbol_name: symbol_name.to_string(),
                qualified_name: qualified_name.to_string(),
                symbol_kind: "function".to_string(),
                signature: Some("fn foo()".to_string()),
                doc_comment: Some("A test function".to_string()),
                body_preview: Some("let x = 1;".to_string()),
                line_start: 1,
                line_end: 5,
                language: "Rust".to_string(),
            },
            content_hash: format!("hash_{symbol_name}"),
            embedding: vec![0.0f32; 384],
        }
    }

    fn dummy_file(path: &str, chunks: Vec<IndexedChunk>) -> IndexedFile {
        IndexedFile {
            rel_path: path.to_string(),
            content_hash: "abc123".to_string(),
            language: "Rust".to_string(),
            churn_count: 0,
            hotspot_score: 0.0,
            commits: vec![],
            chunks,
        }
    }

    pub(crate) async fn test_logic_version_roundtrip(store: Arc<dyn CodeIndexStore>) -> Result<()> {
        assert_eq!(store.stored_logic_version().await?, None);
        store.set_stored_logic_version(5).await?;
        assert_eq!(store.stored_logic_version().await?, Some(5));
        store.set_stored_logic_version(10).await?;
        assert_eq!(store.stored_logic_version().await?, Some(10));
        Ok(())
    }

    pub(crate) async fn test_replace_and_search(store: Arc<dyn CodeIndexStore>) -> Result<()> {
        let file = dummy_file(
            "src/test.rs",
            vec![
                dummy_chunk("my_function", "crate::my_function"),
                dummy_chunk("MyStruct", "crate::MyStruct"),
            ],
        );
        store.replace_file(file).await?;

        let hash = store.file_content_hash("src/test.rs").await?;
        assert_eq!(hash.as_deref(), Some("abc123"));

        let results = store.search_code(vec![0.0f32; 384], "my_function", 5).await?;
        assert!(!results.is_empty());
        Ok(())
    }

    pub(crate) async fn test_get_symbol(store: Arc<dyn CodeIndexStore>) -> Result<()> {
        let file = dummy_file(
            "src/test.rs",
            vec![dummy_chunk("unique_func", "crate::unique_func")],
        );
        store.replace_file(file).await?;

        let results = store.get_symbol("unique_func", None).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].symbol_name, "unique_func");

        let results = store.get_symbol("crate::unique_func", None).await?;
        assert_eq!(results.len(), 1);

        let results = store.get_symbol("unique_func", Some("src/test.rs")).await?;
        assert_eq!(results.len(), 1);

        let results = store.get_symbol("unique_func", Some("src/other.rs")).await?;
        assert_eq!(results.len(), 0);
        Ok(())
    }

    pub(crate) async fn test_clear_all(store: Arc<dyn CodeIndexStore>) -> Result<()> {
        let file = dummy_file("src/test.rs", vec![dummy_chunk("f", "crate::f")]);
        store.replace_file(file).await?;
        store.set_stored_logic_version(1).await?;

        store.clear_all().await?;

        assert_eq!(store.stored_logic_version().await?, Some(1));
        assert_eq!(store.file_content_hash("src/test.rs").await?, None);
        let results = store.get_symbol("f", None).await?;
        assert!(results.is_empty());
        Ok(())
    }

    pub(crate) async fn test_remove_file(store: Arc<dyn CodeIndexStore>) -> Result<()> {
        let file = dummy_file("src/test.rs", vec![dummy_chunk("f", "crate::f")]);
        store.replace_file(file).await?;

        store.remove_file("src/test.rs").await?;
        assert_eq!(store.file_content_hash("src/test.rs").await?, None);
        let results = store.get_symbol("f", None).await?;
        assert!(results.is_empty());
        Ok(())
    }

    pub(crate) async fn test_record_commit(store: Arc<dyn CodeIndexStore>) -> Result<()> {
        let file = dummy_file("src/test.rs", vec![dummy_chunk("f", "crate::f")]);
        store.replace_file(file).await?;

        let commit = CommitInfo {
            sha: "abc123def456".to_string(),
            message: "test commit".to_string(),
        };
        let updates = vec![("src/test.rs".to_string(), 3i64, 0.5f64)];
        store.record_commit(&commit, &updates).await?;

        let results = store.get_symbol("f", None).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].churn_count, 3);
        assert!((results[0].hotspot_score - 0.5).abs() < 0.001);
        assert!(!results[0].related_commits.is_empty());
        Ok(())
    }

    pub(crate) async fn test_zero_chunk_replace_clears_stale_data(store: Arc<dyn CodeIndexStore>) -> Result<()> {
        let file = dummy_file("src/test.rs", vec![dummy_chunk("old_func", "crate::old_func")]);
        store.replace_file(file).await?;

        let results = store.get_symbol("old_func", None).await?;
        assert_eq!(results.len(), 1);

        let empty = dummy_file("src/test.rs", vec![]);
        store.replace_file(empty).await?;

        let hash = store.file_content_hash("src/test.rs").await?;
        assert_eq!(hash.as_deref(), Some("abc123"));

        let results = store.get_symbol("old_func", None).await?;
        assert!(results.is_empty());
        Ok(())
    }
}
