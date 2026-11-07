//! MB-0006 (P2) and repo-level coverage of the store view.

use uuid::Uuid;

use crate::CoreError;
use crate::domain::Direction;
use crate::repo::{CategoryRepo, ProductRepo, ShoppingListRepo, SqlitePool};

use super::{make_category, make_list, make_user, test_db};

/// `(category name, [item text])` per store-view group.
async fn store_shape(
    pool: &SqlitePool,
    owner: Uuid,
    list: Uuid,
) -> Vec<(Option<String>, Vec<String>)> {
    ShoppingListRepo::store_view(pool, owner, list)
        .await
        .unwrap()
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name),
                g.items.into_iter().map(|i| i.text).collect(),
            )
        })
        .collect()
}

fn group(category: Option<&str>, items: &[&str]) -> (Option<String>, Vec<String>) {
    (
        category.map(str::to_owned),
        items.iter().map(|&s| s.to_owned()).collect(),
    )
}

/// MB-0006 P2: within a category, items follow the user's product order;
/// items without one (free text, or a product since moved to another
/// category) follow by name, ignoring case, after them.
#[tokio::test]
async fn view_order_within_category() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let list = make_list(&pool, u).await;

    // The user's order in Hevi: Tomaatti, Kurkku, Omena (not alphabetical).
    let mut products = Vec::new();
    for name in ["Tomaatti", "Kurkku", "Omena"] {
        products.push(ProductRepo::create(&pool, u, name, hevi).await.unwrap().id);
    }
    // Added to the list in another order, plus free text put in Hevi.
    for product in [products[2], products[0], products[1]] {
        ShoppingListRepo::add_product(&pool, u, list, product, None)
            .await
            .unwrap();
    }
    for text in ["sipuli", "Banaani"] {
        let item = ShoppingListRepo::add_item(&pool, u, list, text, None)
            .await
            .unwrap();
        ShoppingListRepo::assign_category(&pool, u, list, item.id, hevi)
            .await
            .unwrap();
    }
    assert_eq!(
        store_shape(&pool, u, list).await,
        [group(
            Some("Hevi"),
            &["Tomaatti", "Kurkku", "Omena", "Banaani", "sipuli"]
        )]
    );

    // Reordering the products reorders the view.
    ProductRepo::move_within_category(&pool, u, products[2], Direction::Up)
        .await
        .unwrap();
    assert_eq!(
        store_shape(&pool, u, list).await,
        [group(
            Some("Hevi"),
            &["Tomaatti", "Omena", "Kurkku", "Banaani", "sipuli"]
        )]
    );

    // A product moved to another category keeps its item in Hevi (the
    // snapshot), where it no longer has a position: it sorts by name.
    ProductRepo::recategorize(&pool, u, products[0], kylma)
        .await
        .unwrap();
    assert_eq!(
        store_shape(&pool, u, list).await,
        [group(
            Some("Hevi"),
            &["Omena", "Kurkku", "Banaani", "sipuli", "Tomaatti"]
        )]
    );

    // Equal names keep their list order, deterministically.
    ShoppingListRepo::add_product(&pool, u, list, products[1], Some("2"))
        .await
        .unwrap();
    let kurkut: Vec<_> = ShoppingListRepo::store_view(&pool, u, list).await.unwrap()[0]
        .items
        .iter()
        .filter(|i| i.text == "Kurkku")
        .map(|i| i.quantity.clone())
        .collect();
    assert_eq!(kurkut, [None, Some("2".to_owned())]);
}

/// Groups follow the user's category order, uncategorized last; removed
/// items are left out; checking persists; all of it is owner-scoped.
#[tokio::test]
async fn store_view_groups_and_checks() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    make_category(&pool, u, "Pakasteet").await;
    let list = make_list(&pool, u).await;
    let maito = ProductRepo::create(&pool, u, "Maito", kylma).await.unwrap();
    let omena = ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    ShoppingListRepo::add_item(&pool, u, list, "Kynttilät", None)
        .await
        .unwrap();
    ShoppingListRepo::add_product(&pool, u, list, omena.id, None)
        .await
        .unwrap();
    let milk = ShoppingListRepo::add_product(&pool, u, list, maito.id, None)
        .await
        .unwrap();
    let gone = ShoppingListRepo::add_item(&pool, u, list, "Sokeri", None)
        .await
        .unwrap();
    ShoppingListRepo::remove_item(&pool, u, list, gone.id)
        .await
        .unwrap();

    assert_eq!(
        store_shape(&pool, u, list).await,
        [
            group(Some("Kylmä"), &["Maito"]),
            group(Some("Hevi"), &["Omena"]),
            group(None, &["Kynttilät"]),
        ],
        "category order, empty Pakasteet omitted, uncategorized last, removed hidden"
    );
    CategoryRepo::reorder(&pool, u, &[hevi, kylma])
        .await
        .unwrap();
    assert_eq!(
        store_shape(&pool, u, list).await[0].0.as_deref(),
        Some("Hevi")
    );

    // Check-off sets (not flips) the state, and persists.
    for checked in [true, true, false, true] {
        let item = ShoppingListRepo::set_checked(&pool, u, list, milk.id, checked)
            .await
            .unwrap();
        assert_eq!(item.checked, checked);
    }
    let view = ShoppingListRepo::store_view(&pool, u, list).await.unwrap();
    let checked: Vec<_> = view
        .iter()
        .flat_map(|g| &g.items)
        .map(|i| (i.text.as_str(), i.checked))
        .collect();
    assert_eq!(
        checked,
        [("Omena", false), ("Maito", true), ("Kynttilät", false)]
    );

    // A removed item can't be checked; another user can do neither.
    assert!(matches!(
        ShoppingListRepo::set_checked(&pool, u, list, gone.id, true).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ShoppingListRepo::store_view(&pool, other, list).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ShoppingListRepo::set_checked(&pool, other, list, milk.id, false).await,
        Err(CoreError::NotFound)
    ));
    assert!(
        ShoppingListRepo::items(&pool, u, list, false)
            .await
            .unwrap()
            .iter()
            .any(|i| i.id == milk.id && i.checked)
    );
}
