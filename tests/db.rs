use coree::store::{StoreRequest, new_write_lock};
use turso::Connection;

fn dummy_embedding() -> Vec<f32> {
    vec![0.1f32; 384]
}

fn basic_request(content: &str) -> StoreRequest {
    StoreRequest {
        content: content.to_string(),
        memory_type: "decision".to_string(),
        title: "Test memory".to_string(),
        tags: vec![],
        topic_key: None,
        project_id: "test-project".to_string(),
        session_id: "test-session".to_string(),
        importance: Some(0.7),
        facts: vec![],
        source: None,
        pinned: None,
        git_ref: None,
        git_author: None,
    }
}

async fn seed_memory(conn: &Connection, id: &str, project_id: &str) {
    conn.execute(
        "INSERT INTO memories \
         (id, project_id, type, title, content, created_at, updated_at, content_hash) \
         VALUES (?1, ?2, 'fact', 'Title', 'Content', '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z', 'hash')",
        (id.to_string(), project_id.to_string()),
    )
    .await
    .unwrap();
}

struct TestDb {
    conn: Connection,
    #[allow(dead_code)]
    _db: turso::Database,
}

async fn setup() -> TestDb {
    let db = turso::Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();
    coree::migrations::run(&conn).await.unwrap();
    TestDb { conn, _db: db }
}

// --- migrations ---

#[tokio::test]
async fn migrations_run_on_fresh_db() {
    setup().await;
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let db = setup().await;
    coree::migrations::run(&db.conn).await.unwrap();
}

// Each migration is recorded exactly once in schema_migrations, and re-running
// migrations (or a duplicate row arriving from another replica via INSERT OR
// IGNORE) never produces a duplicate bookkeeping row.
#[tokio::test]
async fn migrations_bookkeeping_has_no_duplicates() {
    let db = setup().await;
    coree::migrations::run(&db.conn).await.unwrap();

    // Simulate a row syncing in from another replica: INSERT OR IGNORE of an
    // already-present migration name must be a no-op, not an error.
    db.conn
        .execute(
            "INSERT OR IGNORE INTO schema_migrations (name, applied_at, checksum) \
             VALUES ('v001_initial', 'x', 'x')",
            (),
        )
        .await
        .unwrap();

    let mut rows = db
        .conn
        .query(
            "SELECT COUNT(*) FROM (SELECT name FROM schema_migrations GROUP BY name HAVING COUNT(*) > 1)",
            (),
        )
        .await
        .unwrap();
    let dupes: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(dupes, 0, "schema_migrations must have no duplicate names");
}

// --- delete_batch ---

#[tokio::test]
async fn delete_batch_soft_deletes() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;
    seed_memory(&db.conn, "id-b", "test-project").await;

    let ids = vec!["id-a".to_string(), "id-b".to_string()];
    let n = coree::retrieve::delete_batch(&db.conn, &ids, "test-project")
        .await
        .unwrap();
    assert_eq!(n, 2);

    let results = coree::retrieve::get_full_batch(&db.conn, &ids, "test-project")
        .await
        .unwrap();
    assert!(results.iter().all(|m| m.status == "deleted"));
}

#[tokio::test]
async fn delete_batch_missing_ids_not_counted() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string(), "nonexistent".to_string()];
    let n = coree::retrieve::delete_batch(&db.conn, &ids, "test-project")
        .await
        .unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn delete_batch_is_isolated_by_project_id() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    let n = coree::retrieve::delete_batch(&db.conn, &ids, "other-project")
        .await
        .unwrap();
    assert_eq!(
        n, 0,
        "foreign project must not delete another project's memory"
    );

    let results = coree::retrieve::get_full_batch(&db.conn, &ids, "test-project")
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, "active");
}

// --- get_full_batch ---

#[tokio::test]
async fn get_full_batch_returns_all_found() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;
    seed_memory(&db.conn, "id-b", "test-project").await;

    let ids = vec!["id-a".to_string(), "id-b".to_string()];
    let results = coree::retrieve::get_full_batch(&db.conn, &ids, "test-project")
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn get_full_batch_is_isolated_by_project_id() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    let results = coree::retrieve::get_full_batch(&db.conn, &ids, "other-project")
        .await
        .unwrap();
    assert!(
        results.is_empty(),
        "foreign project must not read another project's memory"
    );
}

// --- pin_batch ---

#[tokio::test]
async fn pin_batch_pins_and_unpins() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    let n = coree::retrieve::pin_batch(&db.conn, &ids, "test-project", true)
        .await
        .unwrap();
    assert_eq!(n, 1);
    let results = coree::retrieve::get_full_batch(&db.conn, &ids, "test-project")
        .await
        .unwrap();
    assert!(results.iter().all(|m| m.pinned));

    let n = coree::retrieve::pin_batch(&db.conn, &ids, "test-project", false)
        .await
        .unwrap();
    assert_eq!(n, 1);
    let results = coree::retrieve::get_full_batch(&db.conn, &ids, "test-project")
        .await
        .unwrap();
    assert!(results.iter().all(|m| !m.pinned));
}

#[tokio::test]
async fn pin_batch_is_isolated_by_project_id() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    let n = coree::retrieve::pin_batch(&db.conn, &ids, "other-project", true)
        .await
        .unwrap();
    assert_eq!(
        n, 0,
        "foreign project must not pin another project's memory"
    );
}

#[tokio::test]
async fn pin_batch_skips_deleted_memories() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    coree::retrieve::delete_batch(&db.conn, &ids, "test-project")
        .await
        .unwrap();

    let n = coree::retrieve::pin_batch(&db.conn, &ids, "test-project", true)
        .await
        .unwrap();
    assert_eq!(n, 0);
}

// --- list ---

#[tokio::test]
async fn list_returns_stored_memories() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let results = coree::retrieve::list(&db.conn, "test-project", None, &[], 10, 0.0)
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.id == "id-a"));
}

#[tokio::test]
async fn list_type_filter() {
    let db = setup().await;
    // seed_memory inserts type='fact'; confirm filter works
    seed_memory(&db.conn, "id-a", "test-project").await;

    let facts = coree::retrieve::list(&db.conn, "test-project", Some("fact"), &[], 10, 0.0)
        .await
        .unwrap();
    assert!(facts.iter().all(|r| r.memory_type == "fact"));

    let decisions = coree::retrieve::list(&db.conn, "test-project", Some("decision"), &[], 10, 0.0)
        .await
        .unwrap();
    assert!(decisions.is_empty());
}

// --- store + get_full roundtrip ---

#[tokio::test]
async fn store_and_get_full_roundtrip() {
    let db = setup().await;
    let lock = new_write_lock();

    let result = coree::store::store_memory(
        &db.conn,
        dummy_embedding(),
        &lock,
        basic_request("This is a test memory about Rust"),
        30,
    )
    .await
    .unwrap();

    assert!(!result.id.is_empty());
    assert!(!result.upserted);

    let mem =
        coree::retrieve::get_full_batch(&db.conn, std::slice::from_ref(&result.id), "test-project")
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
    assert_eq!(mem.content, "This is a test memory about Rust");
    assert_eq!(mem.memory_type, "decision");
    assert!((mem.importance - 0.7).abs() < 0.001);
}

#[tokio::test]
async fn store_dedup_within_window_returns_same_id() {
    let db = setup().await;
    let lock = new_write_lock();

    let r1 = coree::store::store_memory(
        &db.conn,
        dummy_embedding(),
        &lock,
        basic_request("Duplicate content"),
        30,
    )
    .await
    .unwrap();
    let r2 = coree::store::store_memory(
        &db.conn,
        dummy_embedding(),
        &lock,
        basic_request("Duplicate content"),
        30,
    )
    .await
    .unwrap();

    assert_eq!(
        r1.id, r2.id,
        "same content in same session should deduplicate"
    );
    assert!(!r2.upserted);
}

#[tokio::test]
async fn topic_key_upsert_updates_content() {
    let db = setup().await;
    let lock = new_write_lock();

    let mut req1 = basic_request("Original content");
    req1.topic_key = Some("my-topic".to_string());
    let r1 = coree::store::store_memory(&db.conn, dummy_embedding(), &lock, req1, 30)
        .await
        .unwrap();

    // Different session to bypass dedup window.
    let mut req2 = basic_request("Updated content");
    req2.topic_key = Some("my-topic".to_string());
    req2.session_id = "other-session".to_string();
    let r2 = coree::store::store_memory(&db.conn, dummy_embedding(), &lock, req2, 30)
        .await
        .unwrap();

    assert_eq!(r1.id, r2.id, "upsert should keep the same ID");
    assert!(r2.upserted);

    let mem =
        coree::retrieve::get_full_batch(&db.conn, std::slice::from_ref(&r1.id), "test-project")
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
    assert_eq!(mem.content, "Updated content");
}

// --- related_memories ---

#[tokio::test]
async fn related_memories_excludes_batch_ids() {
    let db = setup().await;
    let lock = new_write_lock();

    let r1 = coree::store::store_memory(
        &db.conn,
        dummy_embedding(),
        &lock,
        basic_request("Rust async programming patterns"),
        30,
    )
    .await
    .unwrap();

    let r2 = coree::store::store_memory(
        &db.conn,
        dummy_embedding(),
        &lock,
        basic_request("Rust async programming best practices"),
        30,
    )
    .await
    .unwrap();

    let batch_ids: std::collections::HashSet<String> =
        [r1.id.clone(), r2.id.clone()].into_iter().collect();

    let related = coree::retrieve::related_memories(
        &db.conn,
        &dummy_embedding(),
        "test-project",
        &batch_ids,
        5,
    )
    .await
    .unwrap();

    for r in &related {
        assert_ne!(
            r.id, r1.id,
            "related list must not contain any memory from the batch (r1)"
        );
        assert_ne!(
            r.id, r2.id,
            "related list must not contain any memory from the batch (r2)"
        );
    }
}

#[tokio::test]
async fn related_memories_empty_exclude_set_works() {
    let db = setup().await;
    let lock = new_write_lock();

    coree::store::store_memory(
        &db.conn,
        dummy_embedding(),
        &lock,
        basic_request("Some memory"),
        30,
    )
    .await
    .unwrap();

    let related = coree::retrieve::related_memories(
        &db.conn,
        &dummy_embedding(),
        "test-project",
        &std::collections::HashSet::new(),
        5,
    )
    .await
    .unwrap();

    assert!(!related.is_empty(), "should find the stored memory when no exclusions");
}

#[tokio::test]
async fn related_memories_respects_limit_and_distance_threshold() {
    let db = setup().await;
    let lock = new_write_lock();

    for i in 0..10 {
        let mut req = basic_request(&format!("Memory number {}", i));
        req.title = format!("Title {}", i);
        coree::store::store_memory(
            &db.conn,
            dummy_embedding(),
            &lock,
            req,
            30,
        )
        .await
        .unwrap();
    }

    let related = coree::retrieve::related_memories(
        &db.conn,
        &dummy_embedding(),
        "test-project",
        &std::collections::HashSet::new(),
        3,
    )
    .await
    .unwrap();

    assert!(
        related.len() <= 3,
        "should respect the limit parameter"
    );
    for r in &related {
        assert!(
            r.distance <= coree::retrieve::RELATED_MAX_DIST,
            "all results must be within RELTED_MAX_DIST threshold"
        );
    }
}

#[tokio::test]
async fn store_redacts_secrets_in_facts_and_tags() {
    let db = setup().await;
    let lock = new_write_lock();

    let mut req = basic_request("Normal content about architecture");
    req.facts = vec![
        "API key: sk-abc123XYZabc123XYZabc for OpenAI".to_string(),
        "GitHub token ghp_aBcDeFgHiJkLmNoPqRsTuVwXyZabcd123456 is used".to_string(),
        "This is a normal fact with no secrets".to_string(),
    ];
    req.tags = vec![
        "api".to_string(),
        "JWT: eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c".to_string(),
    ];

    let result = coree::store::store_memory(
        &db.conn,
        dummy_embedding(),
        &lock,
        req,
        30,
    )
    .await
    .unwrap();

    assert!(!result.id.is_empty());
    assert!(!result.upserted);
    assert_eq!(result.redaction_count, 3, "two facts and one tag should be redacted");

    let mem =
        coree::retrieve::get_full_batch(&db.conn, std::slice::from_ref(&result.id), "test-project")
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

    assert_eq!(mem.content, "Normal content about architecture", "content has no secrets, should be unchanged");
    let facts = mem.facts.as_deref().unwrap_or("");
    assert!(!facts.contains("sk-abc"), "sk- token should be redacted in facts");
    assert!(!facts.contains("ghp_"), "GitHub token should be redacted in facts");
    assert!(facts.contains("[REDACTED]"), "redacted marker should appear in facts");
    assert!(facts.contains("normal fact with no secrets"), "normal fact should be unchanged");
    let tags = mem.tags.as_deref().unwrap_or("");
    assert!(!tags.contains("eyJ"), "JWT in tags should be redacted");
    assert!(tags.contains("[REDACTED]"), "redacted marker should appear in tags");
    assert!(tags.contains("api"), "normal tag should be unchanged");
}
