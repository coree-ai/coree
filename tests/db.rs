/// Integration tests using an in-memory libsql database.
/// These cover store, retrieve, and migrations together.
use memso::{migrations, retrieve, store};

struct TestDb {
    pub conn: libsql::Connection,
    // Keep _db alive: dropping it destroys the in-memory database.
    _db: libsql::Database,
}

async fn migrated_db() -> TestDb {
    let db = libsql::Builder::new_local(":memory:").build().await.unwrap();
    let conn = db.connect().unwrap();
    migrations::run(&conn).await.unwrap();
    TestDb { conn, _db: db }
}

/// A dummy 384-dim embedding (all 0.1). Used where vector content does not matter.
fn dummy_embedding() -> Vec<f32> {
    vec![0.1f32; 384]
}

fn basic_request(content: &str) -> store::StoreRequest {
    store::StoreRequest {
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
    }
}

// --- migrations ---

#[tokio::test]
async fn migrations_run_on_fresh_db() {
    migrated_db().await; // must not panic or error
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let db = migrated_db().await;
    // Running a second time must be a no-op, not an error.
    migrations::run(&db.conn).await.unwrap();
}

// --- store + get_full roundtrip ---

#[tokio::test]
async fn store_and_get_full_roundtrip() {
    let db = migrated_db().await;
    let lock = store::new_write_lock();

    let result = store::store_memory(
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

    let mem = retrieve::get_full(&db.conn, &result.id).await.unwrap().unwrap();
    assert_eq!(mem.content, "This is a test memory about Rust");
    assert_eq!(mem.memory_type, "decision");
    assert!((mem.importance - 0.7).abs() < 0.001);
}

#[tokio::test]
async fn get_full_returns_none_for_unknown_id() {
    let db = migrated_db().await;
    let result = retrieve::get_full(&db.conn, "does-not-exist").await.unwrap();
    assert!(result.is_none());
}

// --- dedup ---

#[tokio::test]
async fn store_dedup_within_window_returns_same_id() {
    let db = migrated_db().await;
    let lock = store::new_write_lock();

    let r1 = store::store_memory(&db.conn, dummy_embedding(), &lock, basic_request("Duplicate content"), 30)
        .await
        .unwrap();
    let r2 = store::store_memory(&db.conn, dummy_embedding(), &lock, basic_request("Duplicate content"), 30)
        .await
        .unwrap();

    assert_eq!(r1.id, r2.id, "same content in same session should deduplicate");
    assert!(!r2.upserted);
}

// --- topic-key upsert ---

#[tokio::test]
async fn topic_key_upsert_updates_content() {
    let db = migrated_db().await;
    let lock = store::new_write_lock();

    let mut req1 = basic_request("Original content");
    req1.topic_key = Some("my-topic".to_string());
    let r1 = store::store_memory(&db.conn, dummy_embedding(), &lock, req1, 30)
        .await
        .unwrap();

    // Use a different session_id to bypass the dedup window.
    let mut req2 = basic_request("Updated content");
    req2.topic_key = Some("my-topic".to_string());
    req2.session_id = "other-session".to_string();
    let r2 = store::store_memory(&db.conn, dummy_embedding(), &lock, req2, 30)
        .await
        .unwrap();

    assert_eq!(r1.id, r2.id, "upsert should keep the same ID");
    assert!(r2.upserted);

    let mem = retrieve::get_full(&db.conn, &r1.id).await.unwrap().unwrap();
    assert_eq!(mem.content, "Updated content");
}

// --- list ---

#[tokio::test]
async fn list_returns_stored_memories() {
    let db = migrated_db().await;
    let lock = store::new_write_lock();
    store::store_memory(&db.conn, dummy_embedding(), &lock, basic_request("Listed memory"), 30)
        .await
        .unwrap();

    let results = retrieve::list(&db.conn, "test-project", None, &[], 10, 0.0)
        .await
        .unwrap();

    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.title == "Test memory"));
}

#[tokio::test]
async fn list_type_filter() {
    let db = migrated_db().await;
    let lock = store::new_write_lock();

    store::store_memory(&db.conn, dummy_embedding(), &lock, basic_request("Decision memory"), 30)
        .await
        .unwrap();

    let mut gotcha_req = basic_request("Gotcha memory");
    gotcha_req.memory_type = "gotcha".to_string();
    gotcha_req.session_id = "s2".to_string();
    store::store_memory(&db.conn, dummy_embedding(), &lock, gotcha_req, 30)
        .await
        .unwrap();

    let decisions = retrieve::list(&db.conn, "test-project", Some("decision"), &[], 10, 0.0)
        .await
        .unwrap();
    assert!(decisions.iter().all(|r| r.memory_type == "decision"));

    let gotchas = retrieve::list(&db.conn, "test-project", Some("gotcha"), &[], 10, 0.0)
        .await
        .unwrap();
    assert!(gotchas.iter().all(|r| r.memory_type == "gotcha"));
}

// --- search_bm25 ---

#[tokio::test]
async fn search_bm25_finds_by_keyword() {
    let db = migrated_db().await;
    let lock = store::new_write_lock();

    let mut req = basic_request("rustaceans love ownership and borrowing");
    req.title = "Rust ownership".to_string();
    store::store_memory(&db.conn, dummy_embedding(), &lock, req, 30)
        .await
        .unwrap();

    let results = retrieve::search_bm25(&db.conn, "ownership", "test-project", 5)
        .await
        .unwrap();

    assert!(!results.is_empty(), "BM25 should find the stored memory by keyword");
    assert!(results.iter().any(|r| r.title == "Rust ownership"));
}
