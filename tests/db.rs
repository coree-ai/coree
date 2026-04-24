use turso::Connection;

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
    tyto::migrations::run(&conn).await.unwrap();
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
    tyto::migrations::run(&db.conn).await.unwrap();
}

// --- delete_batch ---

#[tokio::test]
async fn delete_batch_soft_deletes() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;
    seed_memory(&db.conn, "id-b", "test-project").await;

    let ids = vec!["id-a".to_string(), "id-b".to_string()];
    let n = tyto::retrieve::delete_batch(&db.conn, &ids, "test-project").await.unwrap();
    assert_eq!(n, 2);

    let results = tyto::retrieve::get_full_batch(&db.conn, &ids, "test-project").await.unwrap();
    assert!(results.iter().all(|m| m.status == "deleted"));
}

#[tokio::test]
async fn delete_batch_missing_ids_not_counted() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string(), "nonexistent".to_string()];
    let n = tyto::retrieve::delete_batch(&db.conn, &ids, "test-project").await.unwrap();
    assert_eq!(n, 1);
}

#[tokio::test]
async fn delete_batch_is_isolated_by_project_id() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    let n = tyto::retrieve::delete_batch(&db.conn, &ids, "other-project").await.unwrap();
    assert_eq!(n, 0, "foreign project must not delete another project's memory");

    let results = tyto::retrieve::get_full_batch(&db.conn, &ids, "test-project").await.unwrap();
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
    let results = tyto::retrieve::get_full_batch(&db.conn, &ids, "test-project").await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn get_full_batch_is_isolated_by_project_id() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    let results = tyto::retrieve::get_full_batch(&db.conn, &ids, "other-project").await.unwrap();
    assert!(results.is_empty(), "foreign project must not read another project's memory");
}

// --- pin_batch ---

#[tokio::test]
async fn pin_batch_pins_and_unpins() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    let n = tyto::retrieve::pin_batch(&db.conn, &ids, "test-project", true).await.unwrap();
    assert_eq!(n, 1);
    let results = tyto::retrieve::get_full_batch(&db.conn, &ids, "test-project").await.unwrap();
    assert!(results.iter().all(|m| m.pinned));

    let n = tyto::retrieve::pin_batch(&db.conn, &ids, "test-project", false).await.unwrap();
    assert_eq!(n, 1);
    let results = tyto::retrieve::get_full_batch(&db.conn, &ids, "test-project").await.unwrap();
    assert!(results.iter().all(|m| !m.pinned));
}

#[tokio::test]
async fn pin_batch_is_isolated_by_project_id() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    let n = tyto::retrieve::pin_batch(&db.conn, &ids, "other-project", true).await.unwrap();
    assert_eq!(n, 0, "foreign project must not pin another project's memory");
}

#[tokio::test]
async fn pin_batch_skips_deleted_memories() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let ids = vec!["id-a".to_string()];
    tyto::retrieve::delete_batch(&db.conn, &ids, "test-project").await.unwrap();

    let n = tyto::retrieve::pin_batch(&db.conn, &ids, "test-project", true).await.unwrap();
    assert_eq!(n, 0);
}

// --- list ---

#[tokio::test]
async fn list_returns_stored_memories() {
    let db = setup().await;
    seed_memory(&db.conn, "id-a", "test-project").await;

    let results = tyto::retrieve::list(&db.conn, "test-project", None, &[], 10, 0.0).await.unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.id == "id-a"));
}

#[tokio::test]
async fn list_type_filter() {
    let db = setup().await;
    // seed_memory inserts type='fact'; confirm filter works
    seed_memory(&db.conn, "id-a", "test-project").await;

    let facts = tyto::retrieve::list(&db.conn, "test-project", Some("fact"), &[], 10, 0.0).await.unwrap();
    assert!(facts.iter().all(|r| r.memory_type == "fact"));

    let decisions = tyto::retrieve::list(&db.conn, "test-project", Some("decision"), &[], 10, 0.0).await.unwrap();
    assert!(decisions.is_empty());
}
