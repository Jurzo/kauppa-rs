//! MB-0007 acceptance tests for the shopping-list repository (P1, P2, P4) and
//! MB-0004's for list items (P1–P4), and MB-0005's for adding a recipe (P1–P4).
//! Test-only module: `ShoppingListRepo` lives in `repo` and its SQLite impl in
//! `repo::sqlite`; this module exists so the brief's
//! `cargo test -p kauppa-core repo::list::<name>` checks select these tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use uuid::Uuid;

use crate::CoreError;
use crate::domain::{NewIngredient, RecipeInput};
use crate::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo, SqlitePool, UserRepo};

/// A fresh, isolated in-memory database with migrations applied, plus a raw
/// sqlx pool on the same database for seeding and inspecting rows that have
/// no repo method.
async fn test_db() -> (SqlitePool, sqlx::SqlitePool) {
    let url = format!(
        "sqlite:file:list-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    let options = SqliteConnectOptions::from_str(&url)
        .expect("parse url")
        .foreign_keys(true);
    let raw = SqlitePoolOptions::new()
        .connect_with(options)
        .await
        .expect("connect raw pool");
    (pool, raw)
}

async fn make_user(pool: &SqlitePool, email: &str) -> Uuid {
    UserRepo::create(pool, email, "Test User", "argon2-hash")
        .await
        .expect("create user")
        .id
}

async fn names(pool: &SqlitePool, owner: Uuid) -> Vec<String> {
    ShoppingListRepo::list_for_owner(pool, owner)
        .await
        .unwrap()
        .into_iter()
        .map(|l| l.name)
        .collect()
}

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

// MB-0004 acceptance tests (P1–P4) for list items, plus owner scoping.

async fn make_category(pool: &SqlitePool, owner: Uuid, name: &str) -> Uuid {
    CategoryRepo::create(pool, owner, name)
        .await
        .expect("create category")
        .id
}

async fn make_list(pool: &SqlitePool, owner: Uuid) -> Uuid {
    ShoppingListRepo::create(pool, owner, "Kauppa")
        .await
        .expect("create list")
        .id
}

/// `(text, category name)` of the active items, in order.
async fn shape(pool: &SqlitePool, owner: Uuid, list: Uuid) -> Vec<(String, Option<String>)> {
    ShoppingListRepo::items(pool, owner, list, false)
        .await
        .unwrap()
        .into_iter()
        .map(|i| (i.text, i.category.map(|c| c.name)))
        .collect()
}

fn item(text: &str, category: Option<&str>) -> (String, Option<String>) {
    (text.to_owned(), category.map(str::to_owned))
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

// MB-0005 acceptance tests (P1–P4): adding a recipe to a list.

/// A recipe with the given `(product name, quantity, category)` ingredients.
async fn make_recipe(
    pool: &SqlitePool,
    owner: Uuid,
    title: &str,
    ingredients: &[(&str, Option<&str>, Uuid)],
) -> Uuid {
    let recipe = RecipeRepo::create(
        pool,
        owner,
        &RecipeInput {
            title: title.to_owned(),
            ..RecipeInput::default()
        },
    )
    .await
    .unwrap()
    .id;
    for &(name, quantity, category) in ingredients {
        let new = NewIngredient {
            product_name: name.to_owned(),
            quantity: quantity.map(str::to_owned),
            category_id: Some(category),
        };
        RecipeRepo::add_ingredient(pool, owner, recipe, &new)
            .await
            .unwrap();
    }
    recipe
}

/// `(text, quantity, category name)` of the active items, in order.
async fn full_shape(
    pool: &SqlitePool,
    owner: Uuid,
    list: Uuid,
) -> Vec<(String, Option<String>, Option<String>)> {
    ShoppingListRepo::items(pool, owner, list, false)
        .await
        .unwrap()
        .into_iter()
        .map(|i| (i.text, i.quantity, i.category.map(|c| c.name)))
        .collect()
}

fn line(
    text: &str,
    quantity: Option<&str>,
    category: Option<&str>,
) -> (String, Option<String>, Option<String>) {
    (
        text.to_owned(),
        quantity.map(str::to_owned),
        category.map(str::to_owned),
    )
}

/// MB-0005 P1: adding a recipe appends one item per ingredient, in recipe
/// order after the existing items, each copying the ingredient's product,
/// category, and quantity. Another user's recipe or list is `NotFound`; an
/// empty recipe is `Invalid`; nothing is added in either case.
#[tokio::test]
async fn add_recipe_snapshots_ingredients() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let leivonta = make_category(&pool, u, "Leivonta").await;
    let theirs_category = make_category(&pool, other, "Kylmä").await;
    let list = make_list(&pool, u).await;
    ShoppingListRepo::add_item(&pool, u, list, "Kahvi", None)
        .await
        .unwrap();
    let pancakes = make_recipe(
        &pool,
        u,
        "Pannukakku",
        &[
            ("Maito", Some("5 dl"), kylma),
            ("Vehnäjauho", Some("2½ dl"), leivonta),
            ("Kananmuna", None, kylma),
        ],
    )
    .await;

    let added = ShoppingListRepo::add_recipe(&pool, u, list, pancakes, 1.0)
        .await
        .unwrap();
    assert_eq!(added.len(), 3);
    let detail = RecipeRepo::get(&pool, u, pancakes).await.unwrap();
    for (item, ingredient) in added.iter().zip(&detail.ingredients) {
        assert_eq!(item.product_id, ingredient.product_id);
        assert!(item.product_id.is_some());
    }
    assert_eq!(
        full_shape(&pool, u, list).await,
        [
            line("Kahvi", None, None),
            line("Maito", Some("5 dl"), Some("Kylmä")),
            line("Vehnäjauho", Some("2½ dl"), Some("Leivonta")),
            line("Kananmuna", None, Some("Kylmä")),
        ]
    );

    // Owner scoping, and an empty recipe.
    let their_recipe = make_recipe(
        &pool,
        other,
        "Smoothie",
        &[("Jogurtti", None, theirs_category)],
    )
    .await;
    let their_list = make_list(&pool, other).await;
    let empty = make_recipe(&pool, u, "Tyhjä", &[]).await;
    assert!(matches!(
        ShoppingListRepo::add_recipe(&pool, u, list, their_recipe, 1.0).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ShoppingListRepo::add_recipe(&pool, u, their_list, pancakes, 1.0).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ShoppingListRepo::add_recipe(&pool, u, list, empty, 1.0).await,
        Err(CoreError::Invalid(_))
    ));
    for multiplier in [0.0, -1.0, 101.0, f64::NAN] {
        assert!(matches!(
            ShoppingListRepo::add_recipe(&pool, u, list, pancakes, multiplier).await,
            Err(CoreError::Invalid(_))
        ));
    }
    assert_eq!(full_shape(&pool, u, list).await.len(), 4);
    assert!(
        ShoppingListRepo::items(&pool, other, their_list, false)
            .await
            .unwrap()
            .is_empty()
    );
}

/// MB-0005 P2: adding a recipe only inserts list items. The recipe and the
/// catalog are unchanged, re-adding appends again, and later recipe or
/// catalog edits leave the items already added alone.
#[tokio::test]
async fn add_recipe_is_nondestructive() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let list = make_list(&pool, u).await;
    let soup = make_recipe(
        &pool,
        u,
        "Keitto",
        &[
            ("Porkkana", Some("3"), hevi),
            ("Kerma", Some("2 dl"), kylma),
        ],
    )
    .await;
    let favorite = ProductRepo::create(&pool, u, "Peruna", hevi).await.unwrap();
    let new = NewIngredient {
        product_name: "peruna".to_owned(),
        quantity: Some("500 g".to_owned()),
        category_id: None,
    };
    RecipeRepo::add_ingredient(&pool, u, soup, &new)
        .await
        .unwrap();

    let recipe_before = format!(
        "{:?}",
        RecipeRepo::get(&pool, u, soup).await.unwrap().ingredients
    );
    let catalog = |pool: &SqlitePool| {
        let pool = pool.clone();
        async move {
            let mut products = Vec::new();
            for name in ["Porkkana", "Kerma", "Peruna"] {
                let p = ProductRepo::find_by_name(&pool, u, name)
                    .await
                    .unwrap()
                    .unwrap();
                products.push((p.id, p.name, p.category_id, p.is_favorite, p.position));
            }
            products
        }
    };
    let catalog_before = catalog(&pool).await;

    ShoppingListRepo::add_recipe(&pool, u, list, soup, 2.0)
        .await
        .unwrap();
    ShoppingListRepo::add_recipe(&pool, u, list, soup, 1.0)
        .await
        .unwrap();
    assert_eq!(
        format!(
            "{:?}",
            RecipeRepo::get(&pool, u, soup).await.unwrap().ingredients
        ),
        recipe_before,
        "the recipe, quantities included, is unchanged"
    );
    assert_eq!(
        catalog(&pool).await,
        catalog_before,
        "the catalog is unchanged"
    );
    assert!(
        ProductRepo::find_by_name(&pool, u, "Peruna")
            .await
            .unwrap()
            .unwrap()
            .is_favorite,
        "a favorite stays a favorite"
    );
    let expected = [
        line("Porkkana", Some("6"), Some("Hevi")),
        line("Kerma", Some("4 dl"), Some("Kylmä")),
        line("Peruna", Some("1000 g"), Some("Hevi")),
        line("Porkkana", Some("3"), Some("Hevi")),
        line("Kerma", Some("2 dl"), Some("Kylmä")),
        line("Peruna", Some("500 g"), Some("Hevi")),
    ];
    assert_eq!(full_shape(&pool, u, list).await, expected);

    // Later edits to the recipe and the catalog don't reach the items.
    let carrot = RecipeRepo::get(&pool, u, soup).await.unwrap().ingredients[0].id;
    RecipeRepo::set_ingredient_quantity(&pool, u, soup, carrot, Some("10"))
        .await
        .unwrap();
    RecipeRepo::remove_ingredient(&pool, u, soup, carrot)
        .await
        .unwrap();
    ProductRepo::rename(&pool, u, favorite.id, "Uusi peruna")
        .await
        .unwrap();
    ProductRepo::recategorize(&pool, u, favorite.id, kylma)
        .await
        .unwrap();
    assert_eq!(full_shape(&pool, u, list).await, expected);
}

/// MB-0005 P3: with a multiplier, a quantity's leading number scales and the
/// rest is kept; a quantity without one passes through; none stays none.
#[tokio::test]
async fn add_recipe_quantity_multiplier() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let leivonta = make_category(&pool, u, "Leivonta").await;
    let list = make_list(&pool, u).await;
    let buns = make_recipe(
        &pool,
        u,
        "Pulla",
        &[
            ("Maito", Some("5 dl"), leivonta),
            ("Hiiva", Some("1/2 pss"), leivonta),
            ("Sokeri", Some("1,5 dl"), leivonta),
            ("Kardemumma", Some("1 rkl"), leivonta),
            ("Suola", Some("ripaus"), leivonta),
            ("Voi", None, leivonta),
            ("Kananmuna", Some("1-2"), leivonta),
        ],
    )
    .await;

    ShoppingListRepo::add_recipe(&pool, u, list, buns, 3.0)
        .await
        .unwrap();
    let quantities: Vec<Option<String>> = full_shape(&pool, u, list)
        .await
        .into_iter()
        .map(|(_, q, _)| q)
        .collect();
    let expected = [
        Some("15 dl"),
        Some("1.5 pss"),
        Some("4,5 dl"),
        Some("3 rkl"),
        Some("ripaus"),
        None,
        Some("3-6"),
    ];
    assert_eq!(quantities, expected.map(|q| q.map(str::to_owned)),);

    // Halving works too, and a multiplier of 1 copies quantities verbatim.
    let half = make_list(&pool, u).await;
    ShoppingListRepo::add_recipe(&pool, u, half, buns, 0.5)
        .await
        .unwrap();
    assert_eq!(
        full_shape(&pool, u, half).await[0].1.as_deref(),
        Some("2.5 dl")
    );
    let once = make_list(&pool, u).await;
    ShoppingListRepo::add_recipe(&pool, u, once, buns, 1.0)
        .await
        .unwrap();
    assert_eq!(
        full_shape(&pool, u, once).await[2].1.as_deref(),
        Some("1,5 dl")
    );
}

/// MB-0005 P4: an ingredient whose product is uncategorized — or gone —
/// becomes an uncategorized item instead of being dropped.
#[tokio::test]
async fn add_recipe_handles_uncategorized() {
    let (pool, _raw) = test_db().await;
    let u = make_user(&pool, "u@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let mausteet = make_category(&pool, u, "Mausteet").await;
    let list = make_list(&pool, u).await;
    let recipe = make_recipe(
        &pool,
        u,
        "Kastike",
        &[
            ("Kerma", Some("2 dl"), kylma),
            ("Pippuri", None, mausteet),
            ("Suola", Some("ripaus"), mausteet),
        ],
    )
    .await;
    // Pippuri loses its category; Suola loses its product.
    CategoryRepo::delete(&pool, u, mausteet).await.unwrap();
    let salt = ProductRepo::find_by_name(&pool, u, "Suola")
        .await
        .unwrap()
        .unwrap();
    ProductRepo::delete(&pool, u, salt.id).await.unwrap();

    let added = ShoppingListRepo::add_recipe(&pool, u, list, recipe, 1.0)
        .await
        .unwrap();
    assert_eq!(
        full_shape(&pool, u, list).await,
        [
            line("Kerma", Some("2 dl"), Some("Kylmä")),
            line("Pippuri", None, None),
            line("Suola", Some("ripaus"), None),
        ]
    );
    assert!(
        added[1].product_id.is_some(),
        "still linked, just uncategorized"
    );
    assert_eq!(added[2].product_id, None, "unlinked, kept by name");
}
