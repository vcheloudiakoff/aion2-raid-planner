//! Pragmas, tailles de pools, migrations et `Timestamp` en base (CAP-2).

mod common;

use common::*;
use domain::Timestamp;
use server::infra::sqlite::{self, Db, READ_POOL_SIZE, WRITE_POOL_SIZE};
use sqlx::SqlitePool;

async fn pragma(pool: &SqlitePool, name: &str) -> String {
    let row: (String,) = sqlx::query_as(&format!(
        "SELECT CAST(({}) AS TEXT)",
        format_args!("SELECT * FROM pragma_{name}")
    ))
    .fetch_one(pool)
    .await
    .unwrap();
    row.0
}

#[tokio::test]
async fn file_database_has_required_pragmas_and_pool_sizes() {
    let dir = TempDir::new("pragmas");
    let db = Db::open(sqlite::file_options(&dir.0.join("app.db")))
        .await
        .unwrap();
    db.migrate().await.unwrap();

    assert_eq!(db.write.options().get_max_connections(), WRITE_POOL_SIZE);
    assert_eq!(WRITE_POOL_SIZE, 1);
    assert_eq!(db.read.options().get_max_connections(), READ_POOL_SIZE);
    assert_eq!(READ_POOL_SIZE, 4);

    for pool in [&db.write, &db.read] {
        assert_eq!(pragma(pool, "journal_mode").await, "wal");
        assert_eq!(pragma(pool, "synchronous").await, "1", "NORMAL");
        assert_eq!(pragma(pool, "foreign_keys").await, "1");
        assert_eq!(pragma(pool, "busy_timeout").await, "5000");
    }
    db.close().await;
}

#[tokio::test]
async fn memory_database_shares_one_base_across_pools() {
    let db = memory_db("shared_memory").await;
    // Les migrations passées par le pool d'écriture sont visibles du pool de lecture.
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE name = '_sqlx_migrations'")
            .fetch_one(&db.read)
            .await
            .unwrap();
    assert_eq!(n, 1);
    for pool in [&db.write, &db.read] {
        assert_eq!(pragma(pool, "foreign_keys").await, "1");
        assert_eq!(pragma(pool, "busy_timeout").await, "5000");
    }
}

#[tokio::test]
async fn only_links_table_exists() {
    // Aucune table métier avant l'epic 2 : seuls les liens secrets (1-6).
    let db = memory_db("only_links_table").await;
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    )
    .fetch_all(&db.read)
    .await
    .unwrap();
    assert_eq!(
        tables,
        vec![("_sqlx_migrations".to_owned(),), ("links".to_owned(),)]
    );
}

#[tokio::test]
async fn timestamp_round_trips_as_canonical_text() {
    let db = memory_db("timestamp_round_trip").await;
    let ts: Timestamp = "2026-11-06T10:00:00Z".parse().unwrap();
    let (back, text, kind): (Timestamp, String, String) =
        sqlx::query_as("SELECT ?1, ?1, typeof(?1)")
            .bind(ts)
            .fetch_one(&db.read)
            .await
            .unwrap();
    assert_eq!(back, ts);
    assert_eq!(text, "2026-11-06T10:00:00Z");
    assert_eq!(kind, "text");

    let bad: Result<(Timestamp,), _> = sqlx::query_as("SELECT '2026-11-06T10:00:00+00:00'")
        .fetch_one(&db.read)
        .await;
    assert!(bad.is_err(), "non-canonical text is rejected on decode");
}
