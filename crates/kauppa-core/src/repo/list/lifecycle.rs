//! MB-0007 (P1, P2, P4): creating, ordering, renaming and deleting lists.

use uuid::Uuid;

use crate::CoreError;
use crate::repo::ShoppingListRepo;

use super::{make_user, names, test_db};

/// P1: a user can create several named lists; each is theirs alone.
#[tokio::test]
async fn create_named_owner_scoped() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;

    let weekend = ShoppingListRepo::create(&pool, u, "  Viikonloppu ")
        .await
        .unwrap();
    assert_eq!(weekend.name, "Viikonloppu", "trimmed");
    assert_eq!(weekend.owner_id, u);
    ShoppingListRepo::create(&pool, u, "Arki").await.unwrap();
    // Names needn't be unique.
    ShoppingListRepo::create(&pool, u, "Arki").await.unwrap();
    ShoppingListRepo::create(&pool, other, "Juhlat")
        .await
        .unwrap();

    let mut mine = names(&pool, u).await;
    mine.sort();
    assert_eq!(mine, ["Arki", "Arki", "Viikonloppu"]);
    assert_eq!(names(&pool, other).await, ["Juhlat"]);

    // Opening is owner-scoped too.
    assert_eq!(
        ShoppingListRepo::get(&pool, u, weekend.id)
            .await
            .unwrap()
            .name,
        "Viikonloppu"
    );
    assert!(matches!(
        ShoppingListRepo::get(&pool, other, weekend.id).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ShoppingListRepo::get(&pool, u, Uuid::new_v4()).await,
        Err(CoreError::NotFound)
    ));

    assert!(matches!(
        ShoppingListRepo::create(&pool, u, "   ").await,
        Err(CoreError::Invalid(_))
    ));
    assert_eq!(names(&pool, u).await.len(), 3);
}

/// P2: the index lists a user's lists most recently updated first; a rename
/// counts as an update.
#[tokio::test]
async fn index_ordered_by_updated() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;

    let first = ShoppingListRepo::create(&pool, u, "Eka").await.unwrap();
    let second = ShoppingListRepo::create(&pool, u, "Toka").await.unwrap();
    ShoppingListRepo::create(&pool, u, "Kolmas").await.unwrap();
    assert_eq!(names(&pool, u).await, ["Kolmas", "Toka", "Eka"]);

    let renamed = ShoppingListRepo::rename(&pool, u, first.id, "Eka (muokattu)")
        .await
        .unwrap();
    assert!(renamed.updated_at > first.updated_at);
    assert_eq!(renamed.created_at, first.created_at);
    assert_eq!(names(&pool, u).await, ["Eka (muokattu)", "Kolmas", "Toka"]);

    ShoppingListRepo::rename(&pool, u, second.id, "Toka")
        .await
        .unwrap();
    assert_eq!(names(&pool, u).await, ["Toka", "Eka (muokattu)", "Kolmas"]);
}

/// P4: lists can be renamed and deleted, owner-scoped; deleting a list
/// removes its items through `ON DELETE CASCADE` and leaves other lists' items.
#[tokio::test]
async fn rename_and_delete_cascades() {
    let (pool, raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let doomed = ShoppingListRepo::create(&pool, u, "Vanha").await.unwrap();
    let kept = ShoppingListRepo::create(&pool, u, "Uusi").await.unwrap();

    for (list, text) in [
        (doomed.id, "maito"),
        (doomed.id, "leipä"),
        (kept.id, "kahvi"),
    ] {
        sqlx::query("INSERT INTO list_items (id, list_id, text) VALUES (?1, ?2, ?3)")
            .bind(Uuid::new_v4().to_string())
            .bind(list.to_string())
            .bind(text)
            .execute(&raw)
            .await
            .unwrap();
    }
    let items_of = |list: Uuid| {
        let raw = raw.clone();
        async move {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM list_items WHERE list_id = ?1")
                .bind(list.to_string())
                .fetch_one(&raw)
                .await
                .unwrap()
        }
    };

    // Rename: owner only, non-empty.
    assert_eq!(
        ShoppingListRepo::rename(&pool, u, doomed.id, " Vanhat ")
            .await
            .unwrap()
            .name,
        "Vanhat"
    );
    assert!(matches!(
        ShoppingListRepo::rename(&pool, u, doomed.id, "").await,
        Err(CoreError::Invalid(_))
    ));
    assert!(matches!(
        ShoppingListRepo::rename(&pool, other, doomed.id, "Kaapattu").await,
        Err(CoreError::NotFound)
    ));

    // Delete: another user can't, and nothing changes.
    assert!(matches!(
        ShoppingListRepo::delete(&pool, other, doomed.id).await,
        Err(CoreError::NotFound)
    ));
    assert_eq!(items_of(doomed.id).await, 2);

    ShoppingListRepo::delete(&pool, u, doomed.id).await.unwrap();
    assert!(matches!(
        ShoppingListRepo::get(&pool, u, doomed.id).await,
        Err(CoreError::NotFound)
    ));
    assert_eq!(items_of(doomed.id).await, 0, "items cascade");
    assert_eq!(items_of(kept.id).await, 1, "other lists keep theirs");
    assert_eq!(names(&pool, u).await, ["Uusi"]);
    assert!(matches!(
        ShoppingListRepo::delete(&pool, u, doomed.id).await,
        Err(CoreError::NotFound)
    ));
}
