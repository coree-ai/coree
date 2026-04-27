use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
}

/// Extended commit info with timestamp and change size for hotspot scoring.
#[derive(Debug, Clone)]
pub struct CommitStat {
    pub sha: String,
    pub message: String,
    pub timestamp_unix: i64,
    pub lines_changed: u32,
}

/// Fetch recent significant commits touching a file with timestamps and line counts.
/// Use this instead of `file_commits` when hotspot scoring is needed.
pub fn file_commits_with_stats(repo_root: &Path, file_path: &str, limit: usize) -> Vec<CommitStat> {
    let output = match Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("log")
        .arg(format!("-n{}", limit * 3))
        .arg("--format=format:%H|%at|%s")
        .arg("--numstat")
        .arg("--no-merges")
        .arg("--")
        .arg(file_path)
        .output()
    {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    if !output.status.success() {
        return vec![];
    }

    let stdout = match std::str::from_utf8(&output.stdout) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    parse_commits_with_stats(stdout)
        .into_iter()
        .filter(|c| is_significant(&c.message))
        .take(limit)
        .collect()
}

fn parse_commits_with_stats(output: &str) -> Vec<CommitStat> {
    let mut results = Vec::new();
    let mut current: Option<(String, i64, String)> = None; // (sha, timestamp_unix, message)
    let mut lines_changed: u32 = 0;

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        // Header line: full 40-char SHA|unix_timestamp|message
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() == 3
            && parts[0].len() == 40
            && parts[0].chars().all(|c| c.is_ascii_hexdigit())
        {
            if let Some((sha, ts, msg)) = current.take() {
                results.push(CommitStat {
                    sha,
                    timestamp_unix: ts,
                    message: msg,
                    lines_changed,
                });
                lines_changed = 0;
            }
            let ts = parts[1].parse::<i64>().unwrap_or(0);
            current = Some((parts[0].to_string(), ts, parts[2].to_string()));
            continue;
        }

        // Numstat line: "added\tdeleted\tpath" (binary shows "-\t-\tpath")
        if current.is_some() {
            let cols: Vec<&str> = line.splitn(3, '\t').collect();
            if cols.len() == 3 {
                let added = cols[0].parse::<u32>().unwrap_or(0);
                let deleted = cols[1].parse::<u32>().unwrap_or(0);
                lines_changed += added + deleted;
            }
        }
    }

    if let Some((sha, ts, msg)) = current {
        results.push(CommitStat {
            sha,
            timestamp_unix: ts,
            message: msg,
            lines_changed,
        });
    }

    results
}

/// Compute the temporal hotspot score from a file's commit history.
/// Formula: sum(exp(-ln2 * age_days / 180) * min(lines_changed / 100, 3))
/// Higher = more recently and heavily modified. Half-life = 180 days.
pub fn compute_hotspot_score(commits: &[CommitStat]) -> f64 {
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    commits
        .iter()
        .map(|c| {
            let age_days = (now_unix - c.timestamp_unix).max(0) as f64 / 86400.0;
            let decay = (-std::f64::consts::LN_2 * age_days / 180.0).exp();
            let size_weight = (c.lines_changed as f64 / 100.0).min(3.0);
            decay * size_weight
        })
        .sum()
}

/// Fetch commits that touched a specific line range in a file (git log -L).
/// Returns compact one-line summaries: "sha7 message (author, date)".
/// Returns empty vec if git is unavailable or the range has no history.
pub fn symbol_commits(
    repo_root: &Path,
    file_path: &str,
    line_start: usize,
    line_end: usize,
    limit: usize,
) -> Vec<String> {
    let range = format!("{line_start},{line_end}:{file_path}");
    let output = match Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("log")
        .arg(format!("-L{range}"))
        .arg("--no-patch")
        .arg(format!("-n{limit}"))
        .arg("--format=format:%h %s (%an, %as)")
        .output()
    {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    if !output.status.success() {
        return vec![];
    }

    match std::str::from_utf8(&output.stdout) {
        Ok(s) => s
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.to_string())
            .collect(),
        Err(_) => vec![],
    }
}

/// Fetch recent significant commits touching a file.
/// Returns up to `limit` commits, filtered for significance.
pub fn file_commits(repo_root: &Path, file_path: &str, limit: usize) -> Vec<CommitInfo> {
    let output = match Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("log")
        .arg(format!("-n{}", limit * 2)) // over-fetch to account for filtering
        .arg("--oneline")
        .arg("--no-merges")
        .arg("--")
        .arg(file_path)
        .output()
    {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    if !output.status.success() {
        return vec![];
    }

    let stdout = match std::str::from_utf8(&output.stdout) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    stdout
        .lines()
        .filter_map(|line| {
            let (sha, msg) = line.split_once(' ')?;
            if is_significant(msg) {
                Some(CommitInfo {
                    sha: sha.to_string(),
                    message: msg.to_string(),
                })
            } else {
                None
            }
        })
        .take(limit)
        .collect()
}

/// Returns the SHA and message of the current HEAD commit, or None if not in a git repo.
pub fn head_commit(repo_root: &Path) -> Option<CommitInfo> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("log")
        .arg("-1")
        .arg("--oneline")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = std::str::from_utf8(&output.stdout).ok()?.trim();
    let (sha, msg) = s.split_once(' ')?;
    Some(CommitInfo {
        sha: sha.to_string(),
        message: msg.to_string(),
    })
}

/// Returns relative paths of files changed in HEAD (works for initial commits too).
pub fn files_in_head_commit(repo_root: &Path) -> Vec<String> {
    let output = match Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["diff-tree", "--no-commit-id", "-r", "--name-only", "HEAD"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return vec![],
    };
    if !output.status.success() {
        return vec![];
    }
    let s = match std::str::from_utf8(&output.stdout) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    s.lines()
        .map(|l| l.to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Returns false for noise commits that pollute the history embedding.
pub(crate) fn is_significant(msg: &str) -> bool {
    if msg.len() < 15 {
        return false;
    }
    let lower = msg.to_lowercase();
    let skip_prefixes = [
        "merge",
        "revert",
        "bump",
        "wip",
        "fixup!",
        "squash!",
        "chore: bump",
    ];
    skip_prefixes.iter().all(|p| !lower.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::is_significant;

    #[test]
    fn significant_normal_commit() {
        assert!(is_significant("feat: add user authentication"));
        assert!(is_significant("fix: resolve race condition in indexer"));
        assert!(is_significant(
            "refactor: extract DbReady into separate struct"
        ));
    }

    #[test]
    fn insignificant_too_short() {
        assert!(!is_significant("fix"));
        assert!(!is_significant("wip"));
        assert!(!is_significant("tmp fix"));
        // Exactly 14 chars: below the 15-char threshold
        assert!(!is_significant("short message!"));
    }

    #[test]
    fn insignificant_noise_prefixes() {
        assert!(!is_significant("Merge pull request #42 from foo/bar"));
        assert!(!is_significant("merge branch main into feature/x"));
        assert!(!is_significant("revert \"feat: add something\""));
        assert!(!is_significant("bump version to 1.2.3"));
        assert!(!is_significant("WIP: half-done refactor"));
        assert!(!is_significant("fixup! fix: typo in comment"));
        assert!(!is_significant("squash! feat: add auth"));
        assert!(!is_significant("chore: bump dependencies"));
    }

    #[test]
    fn prefix_match_is_case_insensitive() {
        assert!(!is_significant("MERGE branch main into dev"));
        assert!(!is_significant(
            "Revert previous commit because it broke things"
        ));
        assert!(!is_significant("Bump serde from 1.0.1 to 1.0.2"));
    }

    #[test]
    fn non_noise_prefix_containing_noise_word_is_significant() {
        // "merged" starts with "merge" — must be filtered
        assert!(!is_significant("merged the auth feature into main branch"));
        // "bumping" starts with "bump" — must be filtered
        assert!(!is_significant("bumping all deps to latest versions"));
        // But a commit that merely contains the word mid-sentence is fine
        assert!(is_significant("fix: don't revert index on partial failure"));
    }
}
