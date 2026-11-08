//! MB-0025 acceptance tests for the pool's connection settings (P1–P4).
//!
//! These use temp-file databases: WAL and the busy timeout don't apply to
//! `:memory:`, which the per-repo suites use.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, Instant};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use uuid::Uuid;

use super::{MAX_CONNECTIONS, SqlitePool};
use crate::repo::{ShoppingListRepo, UserRepo};

/// A database file in the system temp dir, removed (with its `-wal`,
/// `-shm` and `-journal` siblings) on drop.
struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kauppa-mb0025-{}.db", Uuid::new_v4()));
        Self { path }
    }

    fn url(&self) -> String {
        format!("sqlite://{}", self.path.display())
    }

    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut name = self.path.clone().into_os_string();
        name.push(suffix);
        PathBuf::from(name)
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for path in [
            self.path.clone(),
            self.sibling("-wal"),
            self.sibling("-shm"),
            self.sibling("-journal"),
        ] {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// A migrated pool on a fresh temp-file database.
async fn file_pool(db: &TempDb) -> SqlitePool {
    let pool = SqlitePool::connect(&db.url()).await.expect("connect pool");
    pool.run_migrations().await.expect("run migrations");
    pool
}

async fn make_user(pool: &SqlitePool, email: &str) -> Uuid {
    UserRepo::create(pool, email, "Test User", "argon2-hash")
        .await
        .expect("create user")
        .id
}

async fn journal_mode(pool: &sqlx::SqlitePool) -> String {
    sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(pool)
        .await
        .expect("read journal_mode")
}

/// P1: every pooled connection reports WAL, `synchronous=NORMAL`, foreign
/// keys on, and a busy timeout of at least 5 s. All connections are held at
/// once so each check runs on a distinct connection.
#[tokio::test]
async fn pragmas_on_every_connection() {
    let db = TempDb::new();
    let pool = file_pool(&db).await;

    let mut conns = Vec::new();
    for _ in 0..MAX_CONNECTIONS {
        conns.push(pool.inner.acquire().await.expect("acquire connection"));
    }
    assert_eq!(pool.inner.size(), MAX_CONNECTIONS);

    for conn in &mut conns {
        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut **conn)
            .await
            .unwrap();
        let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
            .fetch_one(&mut **conn)
            .await
            .unwrap();
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut **conn)
            .await
            .unwrap();
        let busy_timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&mut **conn)
            .await
            .unwrap();
        assert_eq!(journal, "wal");
        assert_eq!(synchronous, 1, "synchronous=NORMAL");
        assert_eq!(foreign_keys, 1);
        assert!(busy_timeout >= 5000, "busy_timeout {busy_timeout} ms");
    }
}

/// P2: 8 tasks each adding 50 items to one list on a file-backed database
/// all succeed, and all 400 items are there afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn concurrent_writes_succeed() {
    const TASKS: usize = 8;
    const ITEMS: usize = 50;

    let db = TempDb::new();
    let pool = file_pool(&db).await;
    let user = make_user(&pool, "writer@example.com").await;
    let list = ShoppingListRepo::create(&pool, user, "Shared")
        .await
        .unwrap()
        .id;

    let mut handles = Vec::new();
    for task in 0..TASKS {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            for item in 0..ITEMS {
                ShoppingListRepo::add_item(&pool, user, list, &format!("t{task}-i{item}"), None)
                    .await
                    .map_err(|e| format!("task {task} item {item}: {e}"))?;
            }
            Ok::<(), String>(())
        }));
    }
    for handle in handles {
        handle
            .await
            .expect("join writer")
            .expect("every write succeeds");
    }

    let items = ShoppingListRepo::items(&pool, user, list, false)
        .await
        .unwrap();
    assert_eq!(items.len(), TASKS * ITEMS);
    for task in 0..TASKS {
        for item in 0..ITEMS {
            let text = format!("t{task}-i{item}");
            assert!(items.iter().any(|i| i.text == text), "missing {text}");
        }
    }
}

/// P3: a read started while another connection holds a write transaction
/// returns within 100 ms. The writer uses `BEGIN EXCLUSIVE`, the strongest
/// write lock: in rollback-journal mode it blocks readers until the busy
/// timeout; in WAL mode readers keep reading the last committed snapshot.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reads_during_write() {
    let db = TempDb::new();
    let pool = file_pool(&db).await;
    let user = make_user(&pool, "reader@example.com").await;
    let list = ShoppingListRepo::create(&pool, user, "Groceries")
        .await
        .unwrap()
        .id;
    ShoppingListRepo::add_item(&pool, user, list, "Maito", None)
        .await
        .unwrap();

    let mut writer = pool.inner.acquire().await.unwrap();
    sqlx::query("BEGIN EXCLUSIVE")
        .execute(&mut *writer)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET display_name = 'Uncommitted' WHERE id = ?1")
        .bind(user.to_string())
        .execute(&mut *writer)
        .await
        .unwrap();

    let started = Instant::now();
    let read = tokio::time::timeout(
        Duration::from_secs(10),
        ShoppingListRepo::items(&pool, user, list, false),
    )
    .await;
    let elapsed = started.elapsed();

    sqlx::query("ROLLBACK").execute(&mut *writer).await.unwrap();

    let items = read.expect("read finishes").expect("read succeeds");
    assert_eq!(items.len(), 1);
    assert!(
        elapsed < Duration::from_millis(100),
        "read took {elapsed:?} during a write transaction"
    );
}

/// P4: a database created in rollback-journal mode opens through
/// `SqlitePool::connect`, switches to WAL, and keeps all its data.
#[tokio::test]
async fn rollback_journal_database_switches_to_wal() {
    let db = TempDb::new();

    // Build the fixture the way the pool used to open it: no journal mode
    // set, so SQLite's default rollback journal (`delete`).
    let options = SqliteConnectOptions::from_str(&db.url())
        .unwrap()
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Delete);
    let raw = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    let legacy = SqlitePool { inner: raw };
    legacy.run_migrations().await.unwrap();
    assert_eq!(journal_mode(&legacy.inner).await, "delete");

    let user = make_user(&legacy, "legacy@example.com").await;
    let list = ShoppingListRepo::create(&legacy, user, "Old list")
        .await
        .unwrap()
        .id;
    for text in ["Leipä", "Juusto", "Kahvi"] {
        ShoppingListRepo::add_item(&legacy, user, list, text, Some("1"))
            .await
            .unwrap();
    }
    legacy.inner.close().await;

    let pool = file_pool(&db).await;
    assert_eq!(journal_mode(&pool.inner).await, "wal");
    assert!(db.sibling("-wal").exists(), "WAL file created");

    let (found, _) = UserRepo::find_by_email(&pool, "legacy@example.com")
        .await
        .unwrap()
        .expect("user kept");
    assert_eq!(found.id, user);
    let lists = ShoppingListRepo::list_for_owner(&pool, user).await.unwrap();
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].name, "Old list");
    let items: Vec<String> = ShoppingListRepo::items(&pool, user, list, false)
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.text)
        .collect();
    assert_eq!(items, ["Leipä", "Juusto", "Kahvi"]);
}
