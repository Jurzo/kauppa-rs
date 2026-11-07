//! MB-0004 (P1–P4): list items — favorites and free text, soft delete, snapshots — plus owner scoping; MB-0021 (P1): editing an item's quantity.

use uuid::Uuid;

use crate::CoreError;
use crate::domain::ListItem;
use crate::repo::{CategoryRepo, ProductRepo, ShoppingListRepo, SqlitePool};

use super::{item, make_category, make_list, make_user, test_db};

/// `(text, category name)` of the active items, in order.
async fn shape(pool: &SqlitePool, owner: Uuid, list: Uuid) -> Vec<(String, Option<String>)> {
    ShoppingListRepo::items(pool, owner, list, false)
        .await
        .unwrap()
        .into_iter()
        .map(|i| (i.text, i.category.map(|c| c.name)))
        .collect()
}

/// MB-0004 P1: adding a favorite creates an item that copies the product's
/// name and category at add-time.
#[tokio::test]
async fn add_favorite_snapshots_category() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let theirs = make_category(&pool, other, "Kylmä").await;
    let list = make_list(&pool, u).await;
    let maito = ProductRepo::create(&pool, u, "Maito", kylma).await.unwrap();
    let their_milk = ProductRepo::create(&pool, other, "Maito", theirs)
        .await
        .unwrap();

    let added = ShoppingListRepo::add_product(&pool, u, list, maito.id, Some(" 2 l "))
        .await
        .unwrap();
    assert_eq!(added.text, "Maito");
    assert_eq!(added.product_id, Some(maito.id));
    assert_eq!(added.category.as_ref().map(|c| c.id), Some(kylma));
    assert_eq!(added.quantity.as_deref(), Some("2 l"));
    assert!(added.removed_at.is_none() && !added.checked);
    let without = ShoppingListRepo::add_product(&pool, u, list, maito.id, None)
        .await
        .unwrap();
    assert_eq!(without.quantity, None);

    // Another user's product can't be added; nothing is.
    for product in [their_milk.id, Uuid::new_v4()] {
        assert!(matches!(
            ShoppingListRepo::add_product(&pool, u, list, product, None).await,
            Err(CoreError::Invalid(_))
        ));
    }
    assert_eq!(
        shape(&pool, u, list).await,
        [item("Maito", Some("Kylmä")), item("Maito", Some("Kylmä"))]
    );
}

/// MB-0004 P2: a free-text item starts uncategorized and can later be put in
/// one of the user's categories, without touching the catalog.
#[tokio::test]
async fn freetext_uncategorized_then_assign() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let koti = make_category(&pool, u, "Koti").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let theirs = make_category(&pool, other, "Koti").await;
    let list = make_list(&pool, u).await;

    let candles = ShoppingListRepo::add_item(&pool, u, list, " Kynttilät ", None)
        .await
        .unwrap();
    assert_eq!(candles.text, "Kynttilät");
    assert!(candles.category.is_none() && candles.product_id.is_none());
    assert!(matches!(
        ShoppingListRepo::add_item(&pool, u, list, "  ", None).await,
        Err(CoreError::Invalid(_))
    ));

    for category in [theirs, Uuid::new_v4()] {
        assert!(matches!(
            ShoppingListRepo::assign_category(&pool, u, list, candles.id, category).await,
            Err(CoreError::Invalid(_))
        ));
    }
    assert_eq!(shape(&pool, u, list).await, [item("Kynttilät", None)]);

    let assigned = ShoppingListRepo::assign_category(&pool, u, list, candles.id, koti)
        .await
        .unwrap();
    assert_eq!(assigned.category.map(|c| c.name).as_deref(), Some("Koti"));

    // Re-assigning a product's item changes only the item.
    let maito = ProductRepo::create(&pool, u, "Maito", kylma).await.unwrap();
    let milk = ShoppingListRepo::add_product(&pool, u, list, maito.id, None)
        .await
        .unwrap();
    ShoppingListRepo::assign_category(&pool, u, list, milk.id, koti)
        .await
        .unwrap();
    assert_eq!(
        shape(&pool, u, list).await,
        [item("Kynttilät", Some("Koti")), item("Maito", Some("Koti"))]
    );
    let catalog = ProductRepo::find_by_name(&pool, u, "Maito")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(catalog.category_id, Some(kylma));
}

/// MB-0004 P3: removing an item is a soft delete — default reads exclude it,
/// it can be listed as removed, and it can be restored to its old place.
#[tokio::test]
async fn soft_delete_excludes_and_restores() {
    let (pool, raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let list = make_list(&pool, u).await;
    let mut ids = Vec::new();
    for text in ["Leipä", "Maito", "Kahvi"] {
        ids.push(
            ShoppingListRepo::add_item(&pool, u, list, text, None)
                .await
                .unwrap()
                .id,
        );
    }

    ShoppingListRepo::remove_item(&pool, u, list, ids[1])
        .await
        .unwrap();
    assert_eq!(
        shape(&pool, u, list).await,
        [item("Leipä", None), item("Kahvi", None)]
    );
    let removed = ShoppingListRepo::items(&pool, u, list, true).await.unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].text, "Maito");
    assert!(removed[0].removed_at.is_some());
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM list_items WHERE list_id = ?1")
        .bind(list.to_string())
        .fetch_one(&raw)
        .await
        .unwrap();
    assert_eq!(rows, 3, "the row is kept");

    // Removing twice, or restoring an active item, is NotFound.
    assert!(matches!(
        ShoppingListRepo::remove_item(&pool, u, list, ids[1]).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ShoppingListRepo::restore_item(&pool, u, list, ids[0]).await,
        Err(CoreError::NotFound)
    ));

    // Items added meanwhile go last; the restored one returns to its place.
    ShoppingListRepo::add_item(&pool, u, list, "Sokeri", None)
        .await
        .unwrap();
    ShoppingListRepo::restore_item(&pool, u, list, ids[1])
        .await
        .unwrap();
    assert_eq!(
        shape(&pool, u, list).await,
        [
            item("Leipä", None),
            item("Maito", None),
            item("Kahvi", None),
            item("Sokeri", None)
        ]
    );
    assert!(
        ShoppingListRepo::items(&pool, u, list, true)
            .await
            .unwrap()
            .is_empty()
    );
}

/// MB-0004 P4: later catalog edits don't change an item. Renaming,
/// recategorizing, or deleting the product leaves its name and category;
/// only the category's own name stays live (RR-0001: membership is frozen).
#[tokio::test]
async fn snapshot_is_stable() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let list = make_list(&pool, u).await;
    let maito = ProductRepo::create(&pool, u, "Maito", kylma).await.unwrap();
    ShoppingListRepo::add_product(&pool, u, list, maito.id, None)
        .await
        .unwrap();

    ProductRepo::rename(&pool, u, maito.id, "Täysmaito")
        .await
        .unwrap();
    ProductRepo::recategorize(&pool, u, maito.id, hevi)
        .await
        .unwrap();
    assert_eq!(shape(&pool, u, list).await, [item("Maito", Some("Kylmä"))]);

    // The category's name is live.
    CategoryRepo::rename(&pool, u, kylma, "Kylmätuotteet")
        .await
        .unwrap();
    assert_eq!(
        shape(&pool, u, list).await,
        [item("Maito", Some("Kylmätuotteet"))]
    );

    // Deleting the product unlinks the item; deleting the category makes it
    // uncategorized. The item itself stays (RR-0001 constraint 4).
    ProductRepo::delete(&pool, u, maito.id).await.unwrap();
    let items = ShoppingListRepo::items(&pool, u, list, false)
        .await
        .unwrap();
    assert_eq!(items[0].product_id, None);
    assert_eq!(items[0].text, "Maito");
    CategoryRepo::delete(&pool, u, kylma).await.unwrap();
    assert_eq!(shape(&pool, u, list).await, [item("Maito", None)]);
}

/// Item methods on another user's list, or with an item from another list,
/// are `NotFound`, and nothing changes.
#[tokio::test]
async fn items_owner_scoped() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let koti = make_category(&pool, u, "Koti").await;
    let mine = make_list(&pool, u).await;
    let also_mine = make_list(&pool, u).await;
    let theirs = make_list(&pool, other).await;
    let candle = ShoppingListRepo::add_item(&pool, u, mine, "Kynttilä", None)
        .await
        .unwrap()
        .id;
    let their_item = ShoppingListRepo::add_item(&pool, other, theirs, "Omena", None)
        .await
        .unwrap()
        .id;
    let maito = ProductRepo::create(&pool, u, "Maito", koti).await.unwrap();

    let not_found = |r: Result<(), CoreError>| matches!(r, Err(CoreError::NotFound));
    assert!(not_found(
        ShoppingListRepo::items(&pool, u, theirs, false)
            .await
            .map(|_| ())
    ));
    assert!(not_found(
        ShoppingListRepo::add_item(&pool, u, theirs, "x", None)
            .await
            .map(|_| ())
    ));
    assert!(not_found(
        ShoppingListRepo::add_product(&pool, u, theirs, maito.id, None)
            .await
            .map(|_| ())
    ));
    for (list, id) in [
        (theirs, their_item),
        (also_mine, candle),
        (mine, their_item),
    ] {
        assert!(not_found(
            ShoppingListRepo::assign_category(&pool, u, list, id, koti)
                .await
                .map(|_| ())
        ));
        assert!(not_found(
            ShoppingListRepo::remove_item(&pool, u, list, id).await
        ));
        assert!(not_found(
            ShoppingListRepo::restore_item(&pool, u, list, id).await
        ));
    }

    let their_items = ShoppingListRepo::items(&pool, other, theirs, false)
        .await
        .unwrap();
    assert_eq!(their_items.len(), 1);
    assert!(their_items[0].category.is_none());
    assert_eq!(shape(&pool, u, mine).await, [item("Kynttilä", None)]);
}

/// Every field of a list's items, active then removed.
async fn snapshot(pool: &SqlitePool, user: Uuid, list: Uuid) -> Vec<ListItem> {
    let mut items = ShoppingListRepo::items(pool, user, list, false)
        .await
        .unwrap();
    items.extend(
        ShoppingListRepo::items(pool, user, list, true)
            .await
            .unwrap(),
    );
    items
}

/// `ListItem` has no `PartialEq`, so items are compared through `Debug`.
fn fields(items: &[ListItem]) -> Vec<String> {
    items.iter().map(|i| format!("{i:?}")).collect()
}

/// MB-0021 P1: an active item's quantity can be set (trimmed) or cleared
/// (`None` or blank); nothing else about it, or about other items, changes.
/// A removed item, or an item of another list, is `NotFound`.
#[tokio::test]
async fn set_item_quantity() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let list = make_list(&pool, u).await;
    let also_mine = make_list(&pool, u).await;
    let theirs = make_list(&pool, other).await;
    let maito = ProductRepo::create(&pool, u, "Maito", kylma).await.unwrap();
    let milk = ShoppingListRepo::add_product(&pool, u, list, maito.id, Some("2 dl"))
        .await
        .unwrap();
    ShoppingListRepo::set_checked(&pool, u, list, milk.id, true)
        .await
        .unwrap();
    let bread = ShoppingListRepo::add_item(&pool, u, list, "Leipä", Some("1"))
        .await
        .unwrap();
    let sugar = ShoppingListRepo::add_item(&pool, u, list, "Sokeri", Some("1 kg"))
        .await
        .unwrap();
    ShoppingListRepo::remove_item(&pool, u, list, sugar.id)
        .await
        .unwrap();
    let their_item = ShoppingListRepo::add_item(&pool, other, theirs, "Omena", Some("3"))
        .await
        .unwrap();
    let mut expected = snapshot(&pool, u, list).await;
    assert_eq!(expected[0].id, milk.id);
    assert_eq!(expected[1].id, bread.id);

    // Set, trimmed: only the quantity changes.
    let updated = ShoppingListRepo::set_item_quantity(&pool, u, list, milk.id, Some("  3 dl "))
        .await
        .unwrap();
    expected[0].quantity = Some("3 dl".to_owned());
    assert_eq!(
        fields(std::slice::from_ref(&updated)),
        fields(&expected[..1])
    );
    assert!(updated.checked && updated.removed_at.is_none());
    assert_eq!(updated.category.as_ref().map(|c| c.id), Some(kylma));
    assert_eq!(fields(&snapshot(&pool, u, list).await), fields(&expected));

    // An item without a quantity can get one.
    let candles = ShoppingListRepo::add_item(&pool, u, also_mine, "Kynttilät", None)
        .await
        .unwrap();
    let got = ShoppingListRepo::set_item_quantity(&pool, u, also_mine, candles.id, Some("2 kpl"))
        .await
        .unwrap();
    assert_eq!(got.quantity.as_deref(), Some("2 kpl"));

    // Cleared with `None` and with blank.
    for clear in [None, Some("   "), Some("")] {
        ShoppingListRepo::set_item_quantity(&pool, u, list, bread.id, Some("5"))
            .await
            .unwrap();
        let cleared = ShoppingListRepo::set_item_quantity(&pool, u, list, bread.id, clear)
            .await
            .unwrap();
        assert_eq!(cleared.quantity, None, "{clear:?}");
    }
    expected[1].quantity = None;
    assert_eq!(fields(&snapshot(&pool, u, list).await), fields(&expected));

    // A removed item, an item of another list, an unknown item, or another
    // user's list: NotFound, and nothing changes.
    for (list_id, item_id) in [
        (list, sugar.id),
        (also_mine, milk.id),
        (list, their_item.id),
        (list, Uuid::new_v4()),
        (theirs, their_item.id),
    ] {
        assert!(
            matches!(
                ShoppingListRepo::set_item_quantity(&pool, u, list_id, item_id, Some("9")).await,
                Err(CoreError::NotFound)
            ),
            "{list_id} {item_id}"
        );
    }
    assert_eq!(fields(&snapshot(&pool, u, list).await), fields(&expected));
    let theirs_now = snapshot(&pool, other, theirs).await;
    assert_eq!(theirs_now[0].quantity.as_deref(), Some("3"));
}
