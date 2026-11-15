//! MB-0058 (RR-0043, RR-0046): saving a product's name and category together.

use uuid::Uuid;

use crate::CoreError;
use crate::repo::ProductRepo;

use super::{make_category, make_user, shape, test_pool};

/// Name and category change in one call; a product that changes group goes to
/// the end of the new one, one that stays keeps its place.
#[tokio::test]
async fn saves_name_and_category_together() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let omena = ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    let banaani = ProductRepo::create(&pool, u, "Banaani", hevi)
        .await
        .unwrap();
    ProductRepo::create(&pool, u, "Maito", kylma).await.unwrap();

    let saved = ProductRepo::update(&pool, u, omena.id, " Vihreä omena ", Some(kylma))
        .await
        .unwrap();
    assert_eq!(saved.name, "Vihreä omena");
    assert_eq!(saved.category_id, Some(kylma));

    // Same category: renamed in place.
    let same = ProductRepo::update(&pool, u, banaani.id, "Banaanit", Some(hevi))
        .await
        .unwrap();
    assert_eq!(same.position, banaani.position);

    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        [
            (Some("Hevi"), vec!["Banaanit"]),
            (Some("Kylmä"), vec!["Maito", "Vihreä omena"]),
        ]
    );
}

/// `None` clears the category: the favorite moves to Uncategorized, last
/// (RR-0046 amends RR-0001 constraint 3).
#[tokio::test]
async fn none_clears_the_category() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let omena = ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    ProductRepo::create(&pool, u, "Kurkku", hevi).await.unwrap();

    let cleared = ProductRepo::update(&pool, u, omena.id, "Omena", None)
        .await
        .unwrap();
    assert_eq!(cleared.category_id, None);
    assert!(cleared.is_favorite, "still a favorite");

    // Clearing again keeps it in place.
    let again = ProductRepo::update(&pool, u, omena.id, "Omenat", None)
        .await
        .unwrap();
    assert_eq!(again.position, cleared.position);

    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        [(Some("Hevi"), vec!["Kurkku"]), (None, vec!["Omenat"])]
    );
}

/// A rejected part changes nothing: a taken name, an empty name, a foreign or
/// unknown category; another owner's product is `NotFound`.
#[tokio::test]
async fn rejection_changes_nothing() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let theirs = make_category(&pool, other, "Pakasteet").await;
    let omena = ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    ProductRepo::create(&pool, u, "Maito", kylma).await.unwrap();

    assert!(matches!(
        ProductRepo::update(&pool, u, omena.id, "MAITO", Some(kylma)).await,
        Err(CoreError::Conflict(_))
    ));
    assert!(matches!(
        ProductRepo::update(&pool, u, omena.id, "  ", Some(kylma)).await,
        Err(CoreError::Invalid(_))
    ));
    assert!(matches!(
        ProductRepo::update(&pool, u, omena.id, "Omppu", Some(theirs)).await,
        Err(CoreError::Invalid(_))
    ));
    assert!(matches!(
        ProductRepo::update(&pool, u, omena.id, "Omppu", Some(Uuid::new_v4())).await,
        Err(CoreError::Invalid(_))
    ));
    assert!(matches!(
        ProductRepo::update(&pool, other, omena.id, "Hax", None).await,
        Err(CoreError::NotFound)
    ));

    let groups = ProductRepo::list_for_owner(&pool, u).await.unwrap();
    assert_eq!(
        shape(&groups),
        [
            (Some("Hevi"), vec!["Omena"]),
            (Some("Kylmä"), vec!["Maito"])
        ]
    );
}
