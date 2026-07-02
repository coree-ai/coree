/// Shared code-search types and formatting helpers, independent of the storage backend.
///
/// Search implementations live in each backend (turso_store, sqlite_store); this
/// module only holds `CodeResult`, the RRF merge + body-hash dedup helpers,
/// `format_result`/`format_result_compact`.
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub(crate) const RRF_K: f64 = 60.0;

pub(crate) fn rrf_score(rank: Option<usize>) -> f64 {
    rank.map(|r| 1.0 / (RRF_K + r as f64)).unwrap_or(0.0)
}

pub(crate) fn body_hash(body: Option<&str>) -> [u8; 32] {
    let normalized = body
        .unwrap_or("")
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n");
    Sha256::digest(normalized.as_bytes()).into()
}

pub(crate) fn merge_and_dedup(mut scored: Vec<CodeResult>, limit: usize) -> Vec<CodeResult> {
    scored.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut seen_hashes: HashMap<[u8; 32], usize> = HashMap::new();
    let mut deduped: Vec<CodeResult> = Vec::new();
    for r in scored {
        let hash = body_hash(r.body_preview.as_deref());
        if let Some(&idx) = seen_hashes.get(&hash) {
            deduped[idx].duplicate_count += 1;
        } else {
            seen_hashes.insert(hash, deduped.len());
            deduped.push(r);
        }
    }
    deduped.truncate(limit);
    deduped
}

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
    pub body_preview: Option<String>,
    pub churn_count: i64,
    pub hotspot_score: f64,
    pub language: String,
    pub rrf_score: f64,
    pub related_commits: Vec<String>,
    /// Number of identical (same body hash) results collapsed into this entry.
    pub duplicate_count: usize,
    /// File ownership: "author (N commits, last DATE)" strings, ranked by commit count.
    pub owners: Vec<String>,
}

/// Format a CodeResult for display to the agent.
pub fn format_result(r: &CodeResult, verbose: bool) -> String {
    let dup_suffix = if r.duplicate_count > 0 {
        format!("  (+{} duplicates)", r.duplicate_count)
    } else {
        String::new()
    };
    let mut out = format!(
        "[{}] {:.3}  {}:{}-{}{} {}\n",
        r.symbol_kind,
        r.rrf_score,
        r.file_path,
        r.line_start,
        r.line_end,
        dup_suffix,
        r.qualified_name
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
            r.related_commits
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !r.owners.is_empty() {
        out.push_str(&format!(
            "Owners: {}\n",
            r.owners.join(", ")
        ));
    }
    out
}

/// Compact one-line code result for the inject path.
pub fn format_result_compact(r: &CodeResult) -> String {
    format!(
        "[{}] {:.3}  {}:{}  {}\n",
        r.symbol_kind, r.rrf_score, r.file_path, r.line_start, r.qualified_name
    )
}
