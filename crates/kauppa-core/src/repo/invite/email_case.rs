//! MB-0044 (RR-0028): email addresses are unique ignoring ASCII case, at
//! the repo layer and in the schema (`0010_email_nocase.sql`).

use std::path::Path;

use chrono::Utc;

use super::{TempDir, raw_pool, test_db};
use crate::repo::{InvitationRepo, SqlitePool, UserRepo};
use crate::{Conflict, CoreError, password, token};

/// P2: registering an address that differs from an existing account's only
/// by case is the duplicate-email conflict, and nothing is written; a
/// lookup in any case finds the account.
#[tokio::test]
async fn duplicate_differs_only_by_case() {
    let (pool, raw) = test_db().await;
    let now = Utc::now();
    UserRepo::create(&pool, "Pat@Example.com", "Pat", "h")
        .await
        .unwrap();

    let created = InvitationRepo::create(&pool, None, None, now)
        .await
        .unwrap();
    let hash = token::hash(created.token.as_str());
    let err = InvitationRepo::register(&pool, &hash, "pat@EXAMPLE.com", "Other", "h", now)
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::Conflict(Conflict::EmailTaken)),
        "{err:?}"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&raw)
        .await
        .unwrap();
    assert_eq!(count, 1, "no second account");

    // `UserRepo::create` hits the same index.
    let err = UserRepo::create(&pool, "PAT@EXAMPLE.COM", "Other", "h")
        .await
        .unwrap_err();
    assert!(
        matches!(err, CoreError::Conflict(Conflict::EmailTaken)),
        "{err:?}"
    );

    for typed in ["Pat@Example.com", "pat@example.com", "PAT@EXAMPLE.COM"] {
        let (user, _) = pool.find_by_email(typed).await.unwrap().expect(typed);
        assert_eq!(user.email, "Pat@Example.com", "stored as typed");
    }
    assert!(
        pool.find_by_email("pat@example.org")
            .await
            .unwrap()
            .is_none()
    );
}

/// Migrates `raw` through the files before 0010 (copies of the committed
/// ones, so the app's full run accepts them afterwards).
async fn migrate_to_0009(dir: &TempDir, raw: &sqlx::SqlitePool) {
    let migrations = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations");
    let old = dir.path().join("migrations");
    std::fs::create_dir_all(&old).unwrap();
    for entry in std::fs::read_dir(&migrations).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.as_str() < "0010" {
            std::fs::copy(&path, old.join(&name)).unwrap();
        }
    }
    sqlx::migrate::Migrator::new(old.as_path())
        .await
        .unwrap()
        .run(raw)
        .await
        .unwrap();
}

/// Every user row, as text, for before/after comparison.
async fn dump(raw: &sqlx::SqlitePool) -> Vec<(String, String, String, String)> {
    sqlx::query_as("SELECT id, email, display_name, password_hash FROM users ORDER BY id")
        .fetch_all(raw)
        .await
        .unwrap()
}

/// P4: a database at 0009 with accounts migrates to 0010 without changing
/// a row, and everyone can still sign in, in any case. A database where two
/// accounts differ only by case refuses the migration (RR-0028 sub-answer
/// 3) and keeps both rows untouched.
#[tokio::test]
async fn migration_keeps_existing_users() {
    let hash = password::hash("correct horse battery staple").unwrap();

    let dir = TempDir::new();
    let url = format!("sqlite://{}", dir.path().join("old.db").display());
    let raw = raw_pool(&url).await;
    migrate_to_0009(&dir, &raw).await;
    let pool = SqlitePool::connect(&url).await.unwrap();
    for email in ["Pat@Example.com", "kim@example.com", "Lee@example.fi"] {
        UserRepo::create(&pool, email, "Someone", &hash)
            .await
            .unwrap();
    }
    let before = dump(&raw).await;
    pool.run_migrations().await.unwrap();
    assert_eq!(dump(&raw).await, before, "no row changed");
    let index: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND name = 'users_email_nocase'",
    )
    .fetch_optional(&raw)
    .await
    .unwrap();
    assert!(index.is_some(), "0010 applied");
    for typed in ["pat@example.com", "KIM@EXAMPLE.COM", "lee@EXAMPLE.fi"] {
        let (_, stored) = pool.find_by_email(typed).await.unwrap().expect(typed);
        assert!(password::verify("correct horse battery staple", &stored).unwrap());
    }

    // Two accounts that differ only by case: the migration fails, nothing
    // is merged or deleted.
    let dir = TempDir::new();
    let url = format!("sqlite://{}", dir.path().join("clash.db").display());
    let raw = raw_pool(&url).await;
    migrate_to_0009(&dir, &raw).await;
    let pool = SqlitePool::connect(&url).await.unwrap();
    UserRepo::create(&pool, "pat@example.com", "Pat", &hash)
        .await
        .unwrap();
    UserRepo::create(&pool, "PAT@example.com", "Pat again", &hash)
        .await
        .unwrap();
    let before = dump(&raw).await;
    assert!(pool.run_migrations().await.is_err(), "refused");
    assert_eq!(dump(&raw).await, before, "both rows kept, unchanged");
    let applied: Vec<i64> = sqlx::query_scalar("SELECT version FROM _sqlx_migrations")
        .fetch_all(&raw)
        .await
        .unwrap();
    assert!(!applied.contains(&10), "0010 not recorded");
}

/// RR-0028 constraint 2: every SQL comparison on `email` in `kauppa-core`
/// names `COLLATE NOCASE`. Scans `repo/sqlite/` for SQL lines (with a
/// `WHERE`, `AND`, `OR`, `ON` or `lower(`) that compare `email` (`email =`,
/// `email=`, `email IN`, `lower(email)`) without it.
#[test]
fn email_comparisons_name_nocase() {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&src.join("repo/sqlite"), &mut files);
    let mut found = Vec::new();
    for file in files {
        if file.file_name().is_some_and(|n| n == "tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&file).unwrap();
        for (i, line) in text.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            // SQL lines only: a comparison sits in a WHERE/ON clause.
            let sql = ["where ", " and ", " or ", " on ", "lower("]
                .iter()
                .any(|k| lower.contains(k));
            let compares = ["email =", "email=", "email in", "lower(email)"]
                .iter()
                .any(|p| lower.contains(p));
            let compares = sql && compares;
            if compares && !lower.contains("collate nocase") {
                found.push(format!("{}:{}: {}", file.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        found.is_empty(),
        "email compared without COLLATE NOCASE:\n{}",
        found.join("\n")
    );
}
