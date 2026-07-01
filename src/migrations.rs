//! Schema migrations for the memory database.
//!
//! Migrations run on every `coree serve` startup against the local (possibly
//! replica) database. One remote database may be shared by several replicas —
//! across projects and even across machines — that each migrate independently
//! and sync the resulting frames. To stay correct under that concurrency, every
//! migration MUST be additive and idempotent:
//!
//! - Create with `CREATE TABLE/INDEX IF NOT EXISTS`.
//! - Add columns with `ALTER TABLE ... ADD COLUMN` guarded against the
//!   `"duplicate column name"` error (see [`apply_v002`]).
//! - Never emit unconditional seed-data `INSERT`s; use `INSERT OR IGNORE`, or
//!   backfill with an idempotent `UPDATE ... WHERE` (see [`apply_v002`]).
//! - Avoid destructive renames; prefer add-new + backfill so an older `coree`
//!   binary sharing the schema keeps working (additive changes are
//!   forward-compatible; a shared remote DB implies a shared schema version).
//!
//! Do NOT edit the SQL of a migration that has already shipped: [`validate_checksum`]
//! hashes each migration's `sql` and warns on mismatch, and existing installs have
//! already applied it. Add a new migration instead.

use anyhow::Result;
use chrono::Utc;
use sha2::{Digest, Sha256};
use turso::Connection;

use crate::{embed, mlog};

struct Migration {
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        name: "v001_initial",
        sql: include_str!("migrations/v001_initial.sql"),
    },
    Migration {
        name: "v002_embed_model",
        sql: include_str!("migrations/v002_embed_model.sql"),
    },
    Migration {
        name: "v003_drop_unused",
        sql: include_str!("migrations/v003_drop_unused.sql"),
    },
    Migration {
        name: "v004_active_topic_key",
        sql: include_str!("migrations/v004_active_topic_key.sql"),
    },
    Migration {
        name: "v005_git_provenance",
        sql: include_str!("migrations/v005_git_provenance.sql"),
    },
];

pub async fn run(conn: &Connection) -> Result<()> {
    // Ensure schema_migrations exists. GOTCHA: Turso/Limbo can return a false
    // "already exists" parse error for CREATE TABLE IF NOT EXISTS even when the
    // table is present (or absent) - see index/schema.rs. Pre-check sqlite_schema
    // by name and issue a bare CREATE only when truly missing, mirroring that
    // workaround. Without this, the bootstrap crashes startup on any already-
    // migrated (e.g. synced replica) database.
    if object_exists(conn, "schema_migrations").await? {
        mlog!("coree: schema_migrations table present");
    } else {
        mlog!("coree: creating schema_migrations table");
        conn.execute(
            "CREATE TABLE schema_migrations (
                name       TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL,
                checksum   TEXT NOT NULL
            )",
            (),
        )
        .await?;
    }

    // DIAGNOSTIC: on the SAME connection that just saw schema_migrations via
    // sqlite_schema (object_exists above), probe a direct query against it.
    // Under turso 0.6.0 this threw a false "no such table: schema_migrations"
    // even though object_exists returned true (the Limbo catalog lie). Logging
    // both sides here makes the serve log show definitively whether the engine
    // bug survives the turso upgrade, without re-reading code.
    match conn.query("SELECT COUNT(*) FROM schema_migrations", ()).await {
        Ok(mut rows) => {
            let n = rows
                .next()
                .await
                .ok()
                .flatten()
                .and_then(|r| r.get::<i64>(0).ok())
                .unwrap_or(-1);
            mlog!("coree: schema_migrations direct-probe ok (rows={n})");
        }
        Err(e) => mlog!("coree: schema_migrations direct-probe FAILED: {e:#}"),
    }

    // Seed schema_migrations from legacy schema_version on first upgrade.
    seed_from_legacy(conn).await?;

    // Apply pending migrations.
    for migration in MIGRATIONS {
        if is_applied(conn, migration.name).await? {
            mlog!("coree: migration {} already applied, skipping", migration.name);
            continue;
        }
        mlog!("coree: applying migration {}", migration.name);
        apply(conn, migration).await?;
        mlog!("coree: migration {} applied", migration.name);
    }

    // Validate checksums of all applied migrations (warn only - DB is already in that state).
    for migration in MIGRATIONS {
        validate_checksum(conn, migration).await?;
    }

    Ok(())
}

/// Returns true if the named migration is recorded in schema_migrations.
async fn is_applied(conn: &Connection, name: &str) -> Result<bool> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM schema_migrations WHERE name = ?1 LIMIT 1",
            (name,),
        )
        .await?;
    Ok(rows.next().await?.is_some())
}

/// Execute a migration's SQL and record it in schema_migrations.
async fn apply(conn: &Connection, migration: &Migration) -> Result<()> {
    match migration.name {
        "v002_embed_model" => apply_v002(conn, migration).await?,
        "v003_drop_unused" => apply_v003(conn, migration).await?,
        "v005_git_provenance" => apply_v005(conn, migration).await?,
        _ => {
            execute_migration_sql(conn, migration.sql).await?;
        }
    }

    let checksum = sha256(migration.sql);
    let now = Utc::now().to_rfc3339();
    // INSERT OR IGNORE, not plain INSERT: when several replicas share one remote
    // database they each apply migrations against their own local replica and push
    // the resulting frames. `name` is the PRIMARY KEY, so two replicas recording
    // the same migration would otherwise collide on merge. Ignoring a duplicate is
    // correct because the DDL above is idempotent — the row is pure bookkeeping.
    // This is a code-only change; it does not alter any migration's `sql`, so the
    // checksums validated in `validate_checksum` are unaffected.
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations (name, applied_at, checksum) VALUES (?1, ?2, ?3)",
        (migration.name, now, checksum),
    )
    .await?;

    Ok(())
}

/// True if an object (table/index/trigger/view) with this name exists.
/// Used to dodge Limbo's false "already exists" error on CREATE ... IF NOT
/// EXISTS (see index/schema.rs) by checking existence before issuing DDL.
async fn object_exists(conn: &Connection, name: &str) -> Result<bool> {
    let mut rows = conn
        .query("SELECT count(*) FROM sqlite_schema WHERE name = ?1", (name,))
        .await?;
    Ok(rows
        .next()
        .await?
        .and_then(|r| r.get::<i64>(0).ok())
        .unwrap_or(0)
        > 0)
}

/// Run a migration's SQL one statement at a time, applying the Limbo
/// false-"already exists" workaround: before a `CREATE <kind> [IF NOT EXISTS]
/// <name>`, skip it when <name> already exists in sqlite_schema. This mirrors
/// the guard in index/schema.rs and also avoids Limbo's extra unreliability of
/// IF NOT EXISTS inside a multi-statement execute_batch. Non-CREATE statements
/// (DROP/ALTER/...) run as-is; statements execute in source order so a
/// DROP-then-recreate (e.g. v004) re-checks existence after its own DROP.
async fn execute_migration_sql(conn: &Connection, sql: &str) -> Result<()> {
    for stmt in split_statements(sql) {
        if let Some(name) = create_target(&stmt)
            && object_exists(conn, &name).await?
        {
            mlog!("coree:   skip (already exists): {}", stmt_summary(&stmt));
            continue;
        }
        mlog!("coree:   exec: {}", stmt_summary(&stmt));
        if let Err(e) = conn.execute(stmt.as_str(), ()).await {
            mlog!("coree:   FAILED: {} -> {e:#}", stmt_summary(&stmt));
            return Err(e.into());
        }
    }
    Ok(())
}

/// Split SQL into individual statements, stripping `/* */` block comments and
/// `--` line comments first. Assumes no `;`, `--`, or `/*` inside string
/// literals, which holds for every migration in this crate.
fn split_statements(sql: &str) -> Vec<String> {
    let no_block = strip_block_comments(sql);
    let no_comments: String = no_block
        .lines()
        .map(|l| match l.find("--") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");
    no_comments
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Remove non-nested `/* */` block comments.
fn strip_block_comments(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut rest = sql;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// If `stmt` is `CREATE <kind> [IF NOT EXISTS] <name> ...`, return <name>.
/// Returns None for non-CREATE statements, which run unguarded.
fn create_target(stmt: &str) -> Option<String> {
    let mut tokens = stmt.split_whitespace();
    if !tokens.next()?.eq_ignore_ascii_case("CREATE") {
        return None;
    }
    let mut tok = tokens.next()?;
    while matches!(
        tok.to_ascii_uppercase().as_str(),
        "UNIQUE" | "VIRTUAL" | "TEMP" | "TEMPORARY"
    ) {
        tok = tokens.next()?;
    }
    if !matches!(
        tok.to_ascii_uppercase().as_str(),
        "TABLE" | "INDEX" | "TRIGGER" | "VIEW"
    ) {
        return None;
    }
    let mut name = tokens.next()?;
    if name.eq_ignore_ascii_case("IF") {
        let _not = tokens.next()?;
        let _exists = tokens.next()?;
        name = tokens.next()?;
    }
    Some(clean_identifier(name))
}

/// Collapse a statement to a single compact line for log readability.
fn stmt_summary(stmt: &str) -> String {
    let one_line = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 120 {
        format!("{}...", one_line.chars().take(120).collect::<String>())
    } else {
        one_line
    }
}

/// Strip surrounding quoting and any glued-on `(` from an identifier token.
fn clean_identifier(raw: &str) -> String {
    raw.split('(')
        .next()
        .unwrap_or(raw)
        .trim_matches(|c| matches!(c, '"' | '`' | '[' | ']' | '\''))
        .to_string()
}

/// v002: ADD COLUMN with "duplicate column name" idempotency, then backfill.
async fn apply_v002(conn: &Connection, _migration: &Migration) -> Result<()> {
    if let Err(e) = conn
        .execute(
            "ALTER TABLE memory_vectors ADD COLUMN embed_model TEXT NOT NULL DEFAULT ''",
            (),
        )
        .await
        && !e.to_string().contains("duplicate column name")
    {
        return Err(anyhow::anyhow!("v002 migration: {e}"));
    }
    conn.execute(
        "UPDATE memory_vectors SET embed_model = ?1 WHERE embed_model = ''",
        (embed::model_id(),),
    )
    .await?;
    Ok(())
}

/// v003: DROP TABLE IF EXISTS is safe; DROP COLUMN with "no such column" idempotency.
async fn apply_v003(conn: &Connection, _migration: &Migration) -> Result<()> {
    conn.execute("DROP TABLE IF EXISTS sessions", ()).await?;
    for col in ["confidence", "supersedes"] {
        let sql = format!("ALTER TABLE memories DROP COLUMN {col}");
        if let Err(e) = conn.execute(&sql, ()).await
            && !e.to_string().contains("no such column")
        {
            return Err(anyhow::anyhow!("v003 migration: {e}"));
        }
    }
    Ok(())
}

/// v005: ADD COLUMN git_ref and git_author with "duplicate column name" idempotency.
async fn apply_v005(conn: &Connection, _migration: &Migration) -> Result<()> {
    for col in ["git_ref", "git_author"] {
        let sql = format!("ALTER TABLE memories ADD COLUMN {col} TEXT");
        if let Err(e) = conn.execute(&sql, ()).await
            && !e.to_string().contains("duplicate column name")
        {
            return Err(anyhow::anyhow!("v005 migration ({col}): {e}"));
        }
    }
    Ok(())
}

/// Seed schema_migrations from the legacy schema_version table on first upgrade.
/// If schema_migrations is already populated this is a no-op.
async fn seed_from_legacy(conn: &Connection) -> Result<()> {
    // Check if schema_migrations already has entries.
    let mut rows = conn
        .query("SELECT COUNT(*) FROM schema_migrations", ())
        .await?;
    let count: i64 = rows
        .next()
        .await?
        .map(|r| r.get::<i64>(0).unwrap_or(0))
        .unwrap_or(0);
    if count > 0 {
        return Ok(());
    }

    // Read legacy version (0 if table doesn't exist yet).
    let legacy_version = get_legacy_version(conn).await?;
    if legacy_version == 0 {
        return Ok(());
    }

    // Mark v001..v00N as applied with a synthetic checksum so the runner skips them.
    let now = Utc::now().to_rfc3339();
    for migration in MIGRATIONS.iter().take(legacy_version as usize) {
        let checksum = sha256(migration.sql);
        conn.execute(
            "INSERT OR IGNORE INTO schema_migrations (name, applied_at, checksum) VALUES (?1, ?2, ?3)",
            (migration.name, now.as_str(), checksum),
        )
        .await?;
    }

    Ok(())
}

/// Read the legacy schema_version value (0 if table or row is absent).
async fn get_legacy_version(conn: &Connection) -> Result<i64> {
    // schema_version may not exist on a fresh DB - handle the error gracefully.
    match conn
        .query("SELECT version FROM schema_version LIMIT 1", ())
        .await
    {
        Ok(mut rows) => Ok(rows
            .next()
            .await?
            .map(|r| r.get::<i64>(0).unwrap_or(0))
            .unwrap_or(0)),
        Err(_) => Ok(0),
    }
}

/// Recompute the checksum of a migration file and log a warning if it differs from stored.
async fn validate_checksum(conn: &Connection, migration: &Migration) -> Result<()> {
    let mut rows = conn
        .query(
            "SELECT checksum FROM schema_migrations WHERE name = ?1 LIMIT 1",
            (migration.name,),
        )
        .await?;
    let stored = match rows.next().await? {
        Some(row) => row.get::<String>(0)?,
        None => return Ok(()), // Not yet applied - no checksum to validate.
    };
    let current = sha256(migration.sql);
    if stored != current {
        mlog!(
            "[coree] WARNING: migration '{}' checksum mismatch (stored={}, current={}). \
             The migration file was modified after it was applied.",
            migration.name,
            stored,
            current
        );
    }
    Ok(())
}

fn sha256(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_target_extracts_names() {
        assert_eq!(
            create_target("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER)").as_deref(),
            Some("schema_version")
        );
        assert_eq!(
            create_target("CREATE UNIQUE INDEX IF NOT EXISTS memories_topic_key ON memories (a)")
                .as_deref(),
            Some("memories_topic_key")
        );
        assert_eq!(
            create_target("CREATE INDEX memories_project_status ON memories (project_id)")
                .as_deref(),
            Some("memories_project_status")
        );
        // Name glued to the column list.
        assert_eq!(
            create_target("CREATE TABLE foo(id TEXT)").as_deref(),
            Some("foo")
        );
        // Non-CREATE statements are unguarded.
        assert_eq!(create_target("DROP INDEX IF EXISTS memories_topic_key"), None);
        assert_eq!(create_target("ALTER TABLE memories ADD COLUMN git_ref TEXT"), None);
    }

    #[test]
    fn split_statements_drops_comments() {
        // v001 ships a /* */ block comment containing CREATE TRIGGER statements
        // and semicolons that must never be executed.
        let sql = "CREATE TABLE a (id TEXT); -- trailing note\n\
                   /* CREATE TRIGGER t AFTER INSERT ON a BEGIN INSERT INTO a VALUES (1); END; */\n\
                   CREATE INDEX a_idx ON a (id);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].starts_with("CREATE TABLE a"));
        assert!(stmts[1].starts_with("CREATE INDEX a_idx"));
        assert!(stmts.iter().all(|s| !s.contains("TRIGGER")));
    }

    #[test]
    fn split_statements_handles_v004_recreate_order() {
        let sql = include_str!("migrations/v004_active_topic_key.sql");
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 3);
        assert!(stmts[0].starts_with("DROP INDEX"));
        assert_eq!(
            create_target(&stmts[1]).as_deref(),
            Some("memories_topic_key")
        );
        assert!(stmts[2].starts_with("DROP TABLE"));
    }
}
