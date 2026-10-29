//! MB-0002 acceptance tests for the product repository (P2–P5), plus the
//! repo-level half of P1. Test-only module: `ProductRepo` lives in `repo` and
//! its SQLite impl in `repo::sqlite`; this module exists so the brief's
//! `cargo test -p kauppa-core repo::product::<name>` checks select these tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use uuid::Uuid;

use crate::CoreError;
use crate::domain::{Direction, ProductGroup};
use crate::repo::{CategoryRepo, ProductRepo, SqlitePool, UserRepo};

/// A fresh, isolated in-memory database with migrations applied.
async fn test_pool() -> SqlitePool {
    let url = format!(
        "sqlite:file:product-{}?mode=memory&cache=shared",
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

async fn make_category(pool: &SqlitePool, owner: Uuid, name: &str) -> Uuid {
    CategoryRepo::create(pool, owner, name)
        .await
        .expect("create category")
        .id
}

/// `(group name, [product names])` per group; `None` is uncategorized.
fn shape(groups: &[ProductGroup]) -> Vec<(Option<&str>, Vec<&str>)> {
    groups
        .iter()
        .map(|g| {
            (
                g.category.as_ref().map(|c| c.name.as_str()),
                g.products.iter().map(|p| p.name.as_str()).collect(),
            )
        })
        .collect()
}

/// P2: product names are unique per owner and non-empty, on create and rename.
#[tokio::test]
async fn unique_nonempty_name() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;

    assert!(matches!(
        ProductRepo::create(&pool, u, "  ", hevi).await,
        Err(CoreError::Invalid(_))
    ));

    let maito = ProductRepo::create(&pool, u, "  Maito ", kylma)
        .await
        .unwrap();
    assert_eq!(maito.name, "Maito", "names are trimmed");
    assert!(maito.is_favorite);

    // Unique per owner, not per category.
    assert!(matches!(
        ProductRepo::create(&pool, u, "Maito", hevi).await,
        Err(CoreError::Conflict(_))
    ));

    let omena = ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    assert!(matches!(
        ProductRepo::rename(&pool, u, omena.id, "Maito").await,
        Err(CoreError::Conflict(_))
    ));
    assert!(matches!(
        ProductRepo::rename(&pool, u, omena.id, "").await,
        Err(CoreError::Invalid(_))
    ));

    // Another owner may reuse the name.
    let other = make_user(&pool, "other@example.com").await;
    let other_cat = make_category(&pool, other, "Kylmä").await;
    ProductRepo::create(&pool, other, "Maito", other_cat)
        .await
        .unwrap();
}

/// P3: products come back grouped by category (in category order,
/// uncategorized last) and ordered by position within each category.
#[tokio::test]
async fn grouped_ordered() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let leivonta = make_category(&pool, u, "Leivonta").await;

    // Interleave categories so grouping can't come from insertion order.
    for (name, category) in [
        ("Maito", kylma),
        ("Omena", hevi),
        ("Jogurtti", kylma),
        ("Banaani", hevi),
        ("Juusto", kylma),
    ] {
        ProductRepo::create(&pool, u, name, category).await.unwrap();
    }
    // An uncategorized product (its category deleted) sorts last.
    ProductRepo::create(&pool, u, "Sokeri", leivonta)
        .await
        .unwrap();
    CategoryRepo::delete(&pool, u, leivonta).await.unwrap();

    // Another owner's products never appear.
    let other = make_user(&pool, "other@example.com").await;
    let other_cat = make_category(&pool, other, "Hevi").await;
    ProductRepo::create(&pool, other, "Peruna", other_cat)
        .await
        .unwrap();

    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        [
            (Some("Hevi"), vec!["Omena", "Banaani"]),
            (Some("Kylmä"), vec!["Maito", "Jogurtti", "Juusto"]),
            (None, vec!["Sokeri"]),
        ]
    );
    for group in &groups {
        assert!(
            group
                .products
                .windows(2)
                .all(|w| w[0].position < w[1].position)
        );
    }

    // Groups follow the user's category order.
    CategoryRepo::reorder(&pool, u, &[kylma, hevi])
        .await
        .unwrap();
    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    let order: Vec<_> = shape(&groups).into_iter().map(|(name, _)| name).collect();
    assert_eq!(order, [Some("Kylmä"), Some("Hevi"), None]);
}

/// P4: deleting a category sets its products' `category_id` to NULL and keeps
/// the product rows.
#[tokio::test]
async fn category_delete_sets_null() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let pakasteet = make_category(&pool, u, "Pakasteet").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let jaatelo = ProductRepo::create(&pool, u, "Jäätelö", pakasteet)
        .await
        .unwrap();
    let herneet = ProductRepo::create(&pool, u, "Herneet", pakasteet)
        .await
        .unwrap();
    ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();

    CategoryRepo::delete(&pool, u, pakasteet).await.unwrap();

    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        [
            (Some("Hevi"), vec!["Omena"]),
            (None, vec!["Jäätelö", "Herneet"]),
        ]
    );
    let uncategorized = &groups[1].products;
    assert!(uncategorized.iter().all(|p| p.category_id.is_none()));
    let ids: Vec<Uuid> = uncategorized.iter().map(|p| p.id).collect();
    assert_eq!(ids, [jaatelo.id, herneet.id], "same rows, not re-created");
}

/// P5: a product can be renamed, recategorized, repositioned within its
/// category, and deleted — and only by its owner.
#[tokio::test]
async fn mutations() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let omena = ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    let banaani = ProductRepo::create(&pool, u, "Banaani", hevi)
        .await
        .unwrap();
    let maito = ProductRepo::create(&pool, u, "Maito", kylma).await.unwrap();

    // Rename.
    let renamed = ProductRepo::rename(&pool, u, omena.id, "Vihreä omena")
        .await
        .unwrap();
    assert_eq!(renamed.name, "Vihreä omena");
    assert!(renamed.updated_at >= omena.updated_at);

    // Reposition within a category; moving past either end is a no-op.
    ProductRepo::move_within_category(&pool, u, banaani.id, Direction::Up)
        .await
        .unwrap();
    ProductRepo::move_within_category(&pool, u, banaani.id, Direction::Up)
        .await
        .unwrap();
    ProductRepo::move_within_category(&pool, u, maito.id, Direction::Down)
        .await
        .unwrap();
    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        [
            (Some("Hevi"), vec!["Banaani", "Vihreä omena"]),
            (Some("Kylmä"), vec!["Maito"]),
        ]
    );

    // Recategorize: appended at the end of the new category; re-choosing the
    // current category keeps its place.
    let moved = ProductRepo::recategorize(&pool, u, banaani.id, kylma)
        .await
        .unwrap();
    assert_eq!(moved.category_id, Some(kylma));
    let same = ProductRepo::recategorize(&pool, u, maito.id, kylma)
        .await
        .unwrap();
    assert_eq!(same.position, maito.position);
    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        [
            (Some("Hevi"), vec!["Vihreä omena"]),
            (Some("Kylmä"), vec!["Maito", "Banaani"]),
        ]
    );

    // Another owner can't touch these products.
    let other = make_user(&pool, "other@example.com").await;
    let other_cat = make_category(&pool, other, "Hevi").await;
    assert!(matches!(
        ProductRepo::rename(&pool, other, maito.id, "Hax").await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ProductRepo::recategorize(&pool, other, maito.id, other_cat).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ProductRepo::move_within_category(&pool, other, banaani.id, Direction::Up).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ProductRepo::delete(&pool, other, maito.id).await,
        Err(CoreError::NotFound)
    ));

    // Delete.
    ProductRepo::delete(&pool, u, maito.id).await.unwrap();
    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        [
            (Some("Hevi"), vec!["Vihreä omena"]),
            (Some("Kylmä"), vec!["Banaani"]),
        ]
    );
    assert!(matches!(
        ProductRepo::delete(&pool, u, maito.id).await,
        Err(CoreError::NotFound)
    ));
}

/// P1 (repo half): a product can't be created in, or moved to, a category the
/// owner doesn't have. The handler-level check is `routes::products`.
#[tokio::test]
async fn foreign_category_rejected() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let mine = make_category(&pool, u, "Hevi").await;
    let theirs = make_category(&pool, other, "Pakasteet").await;

    assert!(matches!(
        ProductRepo::create(&pool, u, "Jäätelö", theirs).await,
        Err(CoreError::Invalid(_))
    ));
    assert!(matches!(
        ProductRepo::create(&pool, u, "Jäätelö", Uuid::new_v4()).await,
        Err(CoreError::Invalid(_))
    ));

    let omena = ProductRepo::create(&pool, u, "Omena", mine).await.unwrap();
    assert!(matches!(
        ProductRepo::recategorize(&pool, u, omena.id, theirs).await,
        Err(CoreError::Invalid(_))
    ));

    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(shape(&groups), [(Some("Hevi"), vec!["Omena"])]);
    assert!(
        ProductRepo::list_for_owner(&pool, other)
            .await
            .unwrap()
            .is_empty()
    );
}
