//! MB-0001 acceptance tests for the category repository (P1–P5). Test-only
//! module: `CategoryRepo` lives in `repo` and its SQLite impl in
//! `repo::sqlite`; this module exists so the brief's
//! `cargo test -p kauppa-core repo::category::<name>` checks select these tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use uuid::Uuid;

use crate::CoreError;
use crate::repo::{CategoryRepo, SqlitePool, UserRepo};

/// A fresh, isolated in-memory database with migrations applied.
async fn test_pool() -> SqlitePool {
    let url = format!(
        "sqlite:file:cat-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    pool
}

async fn make_user(pool: &SqlitePool, email: &str) -> Uuid {
    UserRepo::create(pool, email, "Test User", "argon2-hash")
        .await
        .expect("create user")
        .id
}

/// P1: a repo query for one owner never returns another owner's categories.
#[tokio::test]
async fn owner_isolation() {
    let pool = test_pool().await;
    let a = make_user(&pool, "a@example.com").await;
    let b = make_user(&pool, "b@example.com").await;

    CategoryRepo::create(&pool, a, "Hevi").await.unwrap();
    CategoryRepo::create(&pool, b, "Pakasteet").await.unwrap();

    let a_cats = CategoryRepo::list_for_owner(&pool, a).await.unwrap();
    assert_eq!(a_cats.len(), 1);
    assert_eq!(a_cats[0].name, "Hevi");
    assert!(a_cats.iter().all(|c| c.owner_id == a));
}

/// P2: `list_for_owner` is ordered by position ascending, with name as a stable
/// tiebreak.
#[tokio::test]
async fn list_ordered() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    for name in ["First", "Second", "Third"] {
        CategoryRepo::create(&pool, u, name).await.unwrap();
    }

    let cats = CategoryRepo::list_for_owner(&pool, u).await.unwrap();
    let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["First", "Second", "Third"]);
    assert!(cats.windows(2).all(|w| w[0].position < w[1].position));

    // Tie: reordering only "Third" gives it position 0, shared with "First".
    // Equal positions fall back to name order, the same on every read.
    let third = cats[2].id;
    CategoryRepo::reorder(&pool, u, &[third]).await.unwrap();
    for _ in 0..2 {
        let cats = CategoryRepo::list_for_owner(&pool, u).await.unwrap();
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(cats[0].position, cats[1].position);
        assert_eq!(names, ["First", "Third", "Second"]);
    }
}

/// P3: reorder persists the new position values.
#[tokio::test]
async fn reorder_persists() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let a = CategoryRepo::create(&pool, u, "A").await.unwrap();
    let b = CategoryRepo::create(&pool, u, "B").await.unwrap();
    let c = CategoryRepo::create(&pool, u, "C").await.unwrap();

    CategoryRepo::reorder(&pool, u, &[c.id, a.id, b.id])
        .await
        .unwrap();

    let cats = CategoryRepo::list_for_owner(&pool, u).await.unwrap();
    let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["C", "A", "B"]);
}

/// P4: empty and owner-duplicate names are rejected; the same name is fine
/// for a different owner.
#[tokio::test]
async fn unique_nonempty_name() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;

    assert!(matches!(
        CategoryRepo::create(&pool, u, "   ").await,
        Err(CoreError::Invalid(_))
    ));

    CategoryRepo::create(&pool, u, "Hevi").await.unwrap();
    assert!(matches!(
        CategoryRepo::create(&pool, u, "Hevi").await,
        Err(CoreError::Conflict(_))
    ));

    let other = make_user(&pool, "other@example.com").await;
    CategoryRepo::create(&pool, other, "Hevi").await.unwrap();
}

/// P5: a category can be renamed and deleted (owner-scoped); after deletion
/// it is absent from `list_for_owner`.
#[tokio::test]
async fn rename_and_delete() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let cat = CategoryRepo::create(&pool, u, "Old").await.unwrap();

    let renamed = CategoryRepo::rename(&pool, u, cat.id, "New").await.unwrap();
    assert_eq!(renamed.name, "New");

    // Cross-owner rename / delete must not touch another user's row.
    assert!(matches!(
        CategoryRepo::rename(&pool, other, cat.id, "Hax").await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        CategoryRepo::delete(&pool, other, cat.id).await,
        Err(CoreError::NotFound)
    ));

    CategoryRepo::delete(&pool, u, cat.id).await.unwrap();
    assert!(
        CategoryRepo::list_for_owner(&pool, u)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        CategoryRepo::delete(&pool, u, cat.id).await,
        Err(CoreError::NotFound)
    ));
}
