//! MB-0035 (RR-0023 sub-answer 3): each user's language at the repo layer.
//! Test-only module: the methods live on `UserRepo`, implemented in
//! `repo::sqlite::user`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

use crate::CoreError;
use crate::domain::Language;
use crate::repo::{SqlitePool, UserRepo};

/// A fresh in-memory database with migrations applied, plus a raw sqlx pool
/// on it for writing rows no repo method writes.
async fn test_db() -> (SqlitePool, sqlx::SqlitePool) {
    let url = format!(
        "sqlite:file:language-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    let options = SqliteConnectOptions::from_str(&url).expect("parse url");
    let raw = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("raw pool");
    (pool, raw)
}

#[tokio::test]
async fn language_round_trips_and_defaults_to_none() {
    let (pool, raw) = test_db().await;
    let user = UserRepo::create(&pool, "a@example.com", "A", "hash")
        .await
        .unwrap()
        .id;
    let other = UserRepo::create(&pool, "b@example.com", "B", "hash")
        .await
        .unwrap()
        .id;

    // A new account (and every account from before migration 0009) has
    // chosen nothing.
    assert_eq!(pool.language(user).await.unwrap(), None);

    for lang in Language::ALL {
        pool.set_language(user, Some(lang)).await.unwrap();
        assert_eq!(pool.language(user).await.unwrap(), Some(lang));
        let stored: Option<String> = sqlx::query_scalar("SELECT language FROM users WHERE id = ?1")
            .bind(user.to_string())
            .fetch_one(&raw)
            .await
            .unwrap();
        assert_eq!(stored.as_deref(), Some(lang.tag()));
    }
    // Only that user's row changes.
    assert_eq!(pool.language(other).await.unwrap(), None);

    pool.set_language(user, None).await.unwrap();
    assert_eq!(pool.language(user).await.unwrap(), None);

    // A value core never writes reads as "not chosen".
    sqlx::query("UPDATE users SET language = 'sv' WHERE id = ?1")
        .bind(user.to_string())
        .execute(&raw)
        .await
        .unwrap();
    assert_eq!(pool.language(user).await.unwrap(), None);

    // No such user: reading chooses nothing, writing is NotFound.
    let nobody = Uuid::new_v4();
    assert_eq!(pool.language(nobody).await.unwrap(), None);
    assert!(matches!(
        pool.set_language(nobody, Some(Language::Fi)).await,
        Err(CoreError::NotFound)
    ));
}

/// A directory in the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kauppa-language-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// RR-0023 sub-answer 3 (data-owner): 0009 only adds a nullable column.
/// Migrating a database at 0008 that has accounts changes none of their
/// columns, and every existing account reads "not chosen".
#[tokio::test]
async fn migration_0009_is_additive() {
    let dir = TempDir::new();
    let url = format!("sqlite://{}", dir.0.join("old.db").display());
    let options = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true);
    let raw = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .unwrap();

    // The schema as of 0008, from copies of the committed files.
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let old = dir.0.join("migrations");
    std::fs::create_dir_all(&old).unwrap();
    for entry in std::fs::read_dir(&migrations).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.as_str() < "0009" {
            std::fs::copy(&path, old.join(&name)).unwrap();
        }
    }
    sqlx::migrate::Migrator::new(old.as_path())
        .await
        .unwrap()
        .run(&raw)
        .await
        .unwrap();
    assert!(
        sqlx::query("SELECT language FROM users")
            .fetch_all(&raw)
            .await
            .is_err(),
        "no column before 0009"
    );

    let pool = SqlitePool::connect(&url).await.unwrap();
    let user = UserRepo::create(&pool, "old@example.com", "Old", "hash")
        .await
        .unwrap()
        .id;
    let dump = |raw: sqlx::SqlitePool| async move {
        let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, email, display_name, password_hash, created_at FROM users ORDER BY id",
        )
        .fetch_all(&raw)
        .await
        .unwrap();
        format!("{rows:?}")
    };
    let before = dump(raw.clone()).await;

    pool.run_migrations().await.unwrap();
    assert_eq!(
        dump(raw.clone()).await,
        before,
        "no existing column changed"
    );
    assert_eq!(pool.language(user).await.unwrap(), None);
    let stored: Option<String> = sqlx::query_scalar("SELECT language FROM users WHERE id = ?1")
        .bind(user.to_string())
        .fetch_one(&raw)
        .await
        .unwrap();
    assert_eq!(stored, None);
}
