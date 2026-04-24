use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

use crate::embed;

const RRF_K: f64 = 60.0;

#[derive(Debug)]
pub struct CodeResult {
    pub id: String,
    pub symbol_name: String,
    pub qualified_name: String,
    pub symbol_kind: String,
    pub file_path: String,
    pub line_start: i64,
    pub line_end: i64,
    pub signature: Option<String>,
    pub doc_comment: Option<String>,
    pub churn_count: i64,
    pub hotspot_score: f64,
    pub language: String,
    pub rrf_score: f64,
    pub related_commits: Vec<String>,
}

/// Hybrid vector + FTS search over indexed code chunks.
pub async fn search_code(
    conn: &Arc<turso::Connection>,
    embedding: Vec<f32>,
    query: &str,
    limit: usize,
) -> Result<Vec<CodeResult>> {
    let _blob = embed::floats_to_blob(&embedding);
    let _model = embed::model_id();
    let k = limit * 2;

    // Stream A: vector search (deferred: turso doesn't have native vector ANN extension yet)
    
    // Stream B: Native FTS search
    let fts_ranks: HashMap<String, usize> = {
        let fts_q = build_fts_query(query);
        if fts_q.is_empty() {
            HashMap::new()
        } else {
            let mut rows = conn.query(
                "SELECT id
                 FROM index_chunks
                 WHERE index_chunks_fts MATCH ?1
                 LIMIT ?2",
                (fts_q, k as i64)
            ).await?;
            let mut ranks = HashMap::new();
            let mut i = 0;
            while let Some(row) = rows.next().await? {
                if let Ok(id) = row.get::<String>(0) {
                    ranks.insert(id, i);
                    i += 1;
                }
            }
            ranks
        }
    };

    if fts_ranks.is_empty() {
        return Ok(vec![]);
    }

    let all_ids: Vec<String> = fts_ranks.keys().cloned().collect();

    // Fetch metadata in one query
    let scored: Vec<CodeResult> = {
        let placeholders = all_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT id, symbol_name, qualified_name, symbol_kind, file_path,
                    line_start, line_end, signature, doc_comment, churn_count, hotspot_score, language
             FROM index_chunks WHERE id IN ({placeholders})"
        );
        let mut rows = conn.query(&sql, turso::params_from_iter(all_ids.clone())).await?;
        let mut results = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: String = row.get(0)?;
            let rrf_f = fts_ranks.get(&id).map(|&r| 1.0 / (RRF_K + r as f64)).unwrap_or(0.0);
            results.push(CodeResult {
                id,
                symbol_name: row.get(1)?,
                qualified_name: row.get(2)?,
                symbol_kind: row.get(3)?,
                file_path: row.get(4)?,
                line_start: row.get(5)?,
                line_end: row.get(6)?,
                signature: row.get(7)?,
                doc_comment: row.get(8)?,
                churn_count: row.get(9).unwrap_or(0),
                hotspot_score: row.get(10).unwrap_or(0.0),
                language: row.get(11).unwrap_or_default(),
                rrf_score: rrf_f,
                related_commits: vec![],
            });
        }
        results
    };

    let mut scored = scored;
    scored.sort_by(|a, b| b.rrf_score.partial_cmp(&a.rrf_score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    // Fetch related commits for top results
    for result in &mut scored {
        result.related_commits = fetch_related_commits_sync(conn, &result.id, 3).await?;
    }

    Ok(scored)
}

async fn fetch_related_commits_sync(
    conn: &Arc<turso::Connection>,
    chunk_id: &str,
    limit: usize,
) -> Result<Vec<String>> {
    let mut rows = conn.query(
        "SELECT c.message
         FROM index_commits c
         JOIN index_chunk_commits cc ON cc.commit_sha = c.sha
         WHERE cc.chunk_id = ?1
         LIMIT ?2",
        (chunk_id, limit as i64)
    ).await?;
    let mut msgs = Vec::new();
    while let Some(row) = rows.next().await? {
        if let Ok(msg) = row.get::<String>(0) {
            msgs.push(msg);
        }
    }
    Ok(msgs)
}

/// Lookup a symbol by name (and optionally file path).
pub async fn get_symbol(
    conn: &Arc<turso::Connection>,
    name_path: &str,
    file_path: Option<&str>,
) -> Result<Vec<CodeResult>> {
    let (sql, params): (String, Vec<String>) = if let Some(fp) = file_path {
        (
            "SELECT id, symbol_name, qualified_name, symbol_kind, file_path,
                    line_start, line_end, signature, doc_comment, churn_count, hotspot_score, language
             FROM index_chunks
             WHERE (symbol_name = ?1 OR qualified_name = ?1 OR qualified_name LIKE ?2)
               AND file_path = ?3
             ORDER BY line_start
             LIMIT 20".to_string(),
            vec![name_path.to_string(), format!("%::{name_path}"), fp.to_string()],
        )
    } else {
        (
            "SELECT id, symbol_name, qualified_name, symbol_kind, file_path,
                    line_start, line_end, signature, doc_comment, churn_count, hotspot_score, language
             FROM index_chunks
             WHERE symbol_name = ?1 OR qualified_name = ?1 OR qualified_name LIKE ?2
             ORDER BY hotspot_score DESC, churn_count DESC, line_start
             LIMIT 20".to_string(),
            vec![name_path.to_string(), format!("%::{name_path}")],
        )
    };

    let mut rows = conn.query(&sql, turso::params_from_iter(params)).await?;
    let mut results = Vec::new();
    while let Some(row) = rows.next().await? {
        results.push(CodeResult {
            id: row.get(0)?,
            symbol_name: row.get(1)?,
            qualified_name: row.get(2)?,
            symbol_kind: row.get(3)?,
            file_path: row.get(4)?,
            line_start: row.get(5)?,
            line_end: row.get(6)?,
            signature: row.get(7)?,
            doc_comment: row.get(8)?,
            churn_count: row.get(9).unwrap_or(0),
            hotspot_score: row.get(10).unwrap_or(0.0),
            language: row.get(11).unwrap_or_default(),
            rrf_score: 0.0,
            related_commits: vec![],
        });
    }

    for r in &mut results {
        r.related_commits = fetch_related_commits_sync(conn, &r.id, 5).await?;
    }
    Ok(results)
}

/// Retrieve overall index statistics.
pub async fn index_stats(conn: &Arc<turso::Connection>) -> Result<(i64, i64)> {
    let mut rows = conn.query("SELECT COUNT(*) FROM index_files", ()).await?;
    let files = rows.next().await?.map(|r| r.get::<i64>(0)).transpose()?.unwrap_or(0);
    let mut rows = conn.query("SELECT COUNT(*) FROM index_chunks", ()).await?;
    let chunks = rows.next().await?.map(|r| r.get::<i64>(0)).transpose()?.unwrap_or(0);
    Ok((files, chunks))
}

/// Format a CodeResult for display to the agent.
pub fn format_result(r: &CodeResult, verbose: bool) -> String {
    let mut out = format!(
        "[{}] {:.3}  {}:{}-{} {}\n",
        r.symbol_kind, r.rrf_score, r.file_path, r.line_start, r.line_end, r.qualified_name
    );
    if let Some(ref sig) = r.signature {
        out.push_str(&format!("Signature: {sig}\n"));
    }
    if verbose && let Some(ref doc) = r.doc_comment {
        let first = doc.lines().next().unwrap_or("").trim();
        if !first.is_empty() {
            let cleaned = first
                .trim_start_matches("///")
                .trim_start_matches("//!")
                .trim();
            out.push_str(&format!("Doc: {cleaned}\n"));
        }
    }
    if r.churn_count > 0 {
        out.push_str(&format!("Churn: {} commits\n", r.churn_count));
    }
    if r.hotspot_score > 0.01 {
        out.push_str(&format!("Hotspot: {:.2}\n", r.hotspot_score));
    }
    if !r.related_commits.is_empty() {
        out.push_str(&format!(
            "History: {}\n",
            r.related_commits.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ")
        ));
    }
    out
}

pub(crate) fn build_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter_map(|w| {
            let clean: String = w.chars().filter(|c| c.is_alphanumeric() || *c == '_').collect();
            if clean.is_empty() { None } else { Some(format!("\"{clean}\"*")) }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::build_fts_query;

    #[test]
    fn fts_basic_terms() {
        let q = build_fts_query("index file");
        assert!(q.contains("\"index\"*"));
        assert!(q.contains("\"file\"*"));
    }

    #[test]
    fn fts_strips_special_chars() {
        // Dots, dashes, and other non-alphanumeric chars stripped
        let q = build_fts_query("file.path foo-bar");
        // "file.path" → "filepath" (dot removed)
        assert!(q.contains("\"filepath\"*"));
        // "foo-bar" → "foobar" (dash removed)
        assert!(q.contains("\"foobar\"*"));
    }

    #[test]
    fn fts_empty_tokens_dropped() {
        // A query of only special chars produces no terms
        let q = build_fts_query("... --- !!!");
        assert!(q.is_empty());
    }

    #[test]
    fn fts_single_term() {
        let q = build_fts_query("ownership");
        assert_eq!(q, "\"ownership\"*");
    }

    #[test]
    fn fts_underscores_preserved() {
        // Underscores are alphanumeric-adjacent and kept
        let q = build_fts_query("my_function");
        assert!(q.contains("\"my_function\"*"));
    }

    #[test]
    fn fts_empty_input() {
        assert!(build_fts_query("").is_empty());
        assert!(build_fts_query("   ").is_empty());
    }
}
