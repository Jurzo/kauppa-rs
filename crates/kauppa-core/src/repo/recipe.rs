//! MB-0003 acceptance tests for the recipe repository (P1, P3, P4, P5), plus
//! owner scoping. Test-only module: `RecipeRepo` lives in `repo` and its SQLite
//! impl in `repo::sqlite`; this module exists so the brief's
//! `cargo test -p kauppa-core repo::recipe::<name>` checks select these tests.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use uuid::Uuid;

use crate::CoreError;
use crate::domain::{Direction, NewIngredient, RecipeDetail, RecipeInput};
use crate::repo::{CategoryRepo, ProductRepo, RecipeRepo, SqlitePool, UserRepo};

/// A fresh, isolated in-memory database with migrations applied.
async fn test_pool() -> SqlitePool {
    let url = format!(
        "sqlite:file:recipe-{}?mode=memory&cache=shared",
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

async fn make_recipe(pool: &SqlitePool, owner: Uuid, title: &str) -> Uuid {
    let input = RecipeInput {
        title: title.to_owned(),
        ..RecipeInput::default()
    };
    RecipeRepo::create(pool, owner, &input)
        .await
        .expect("create recipe")
        .id
}

fn ingredient(name: &str, quantity: Option<&str>, category: Option<Uuid>) -> NewIngredient {
    NewIngredient {
        product_name: name.to_owned(),
        quantity: quantity.map(str::to_owned),
        category_id: category,
    }
}

async fn add(pool: &SqlitePool, owner: Uuid, recipe: Uuid, new: NewIngredient) -> Uuid {
    RecipeRepo::add_ingredient(pool, owner, recipe, &new)
        .await
        .expect("add ingredient")
        .id
}

/// Ingredient names in display order.
fn names(detail: &RecipeDetail) -> Vec<&str> {
    detail.ingredients.iter().map(|i| i.text.as_str()).collect()
}

/// P1: creating and editing a recipe persists its title, instructions,
/// optional servings, and an ordered ingredient list.
#[tokio::test]
async fn create_with_ingredients() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let leivonta = make_category(&pool, u, "Leivonta").await;

    let created = RecipeRepo::create(
        &pool,
        u,
        &RecipeInput {
            title: "  Pannukakku ".to_owned(),
            instructions: "Sekoita ja paista.\n".to_owned(),
            servings: Some(4),
        },
    )
    .await
    .unwrap();
    assert_eq!(created.title, "Pannukakku", "title is trimmed");
    add(
        &pool,
        u,
        created.id,
        ingredient("Maito", Some("5 dl"), Some(kylma)),
    )
    .await;
    add(
        &pool,
        u,
        created.id,
        ingredient("Vehnäjauho", Some("2½ dl"), Some(leivonta)),
    )
    .await;

    let detail = RecipeRepo::get(&pool, u, created.id).await.unwrap();
    assert_eq!(detail.recipe.title, "Pannukakku");
    assert_eq!(detail.recipe.instructions, "Sekoita ja paista.");
    assert_eq!(detail.recipe.servings, Some(4));
    assert_eq!(names(&detail), ["Maito", "Vehnäjauho"]);
    let categories: Vec<_> = detail
        .ingredients
        .iter()
        .map(|i| i.category.as_ref().map(|c| c.name.as_str()))
        .collect();
    assert_eq!(categories, [Some("Kylmä"), Some("Leivonta")]);

    // Editing replaces the fields; servings can be cleared.
    let updated = RecipeRepo::update(
        &pool,
        u,
        created.id,
        &RecipeInput {
            title: "Uunipannari".to_owned(),
            instructions: "Paista uunissa.".to_owned(),
            servings: None,
        },
    )
    .await
    .unwrap();
    assert!(updated.updated_at >= created.updated_at);
    let detail = RecipeRepo::get(&pool, u, created.id).await.unwrap();
    assert_eq!(detail.recipe.title, "Uunipannari");
    assert_eq!(detail.recipe.instructions, "Paista uunissa.");
    assert_eq!(detail.recipe.servings, None);
    assert_eq!(names(&detail), ["Maito", "Vehnäjauho"]);

    let listed: Vec<_> = RecipeRepo::list_for_owner(&pool, u)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.title)
        .collect();
    assert_eq!(listed, ["Uunipannari"]);

    // An empty title and servings below 1 are rejected on create and edit.
    for input in [
        RecipeInput {
            title: "  ".to_owned(),
            ..RecipeInput::default()
        },
        RecipeInput {
            title: "Puuro".to_owned(),
            servings: Some(0),
            ..RecipeInput::default()
        },
    ] {
        assert!(matches!(
            RecipeRepo::create(&pool, u, &input).await,
            Err(CoreError::Invalid(_))
        ));
        assert!(matches!(
            RecipeRepo::update(&pool, u, created.id, &input).await,
            Err(CoreError::Invalid(_))
        ));
    }
    assert_eq!(RecipeRepo::list_for_owner(&pool, u).await.unwrap().len(), 1);
}

/// P3: an ingredient for a name with no product creates one — hidden from
/// favorites, in the chosen category — and the same name, in any letter case,
/// reuses it. An existing product keeps its own category.
#[tokio::test]
async fn ingredient_upserts_product() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let theirs = make_category(&pool, other, "Pakasteet").await;
    let pannukakku = make_recipe(&pool, u, "Pannukakku").await;
    let puuro = make_recipe(&pool, u, "Puuro").await;

    let first = RecipeRepo::add_ingredient(
        &pool,
        u,
        pannukakku,
        &ingredient(" Maito ", None, Some(kylma)),
    )
    .await
    .unwrap();
    let product = ProductRepo::find_by_name(&pool, u, "maito")
        .await
        .unwrap()
        .expect("product created");
    assert_eq!(first.product_id, Some(product.id));
    assert_eq!(product.name, "Maito", "stored as typed, trimmed");
    assert_eq!(product.category_id, Some(kylma));
    assert!(
        !product.is_favorite,
        "created from a recipe, not a favorite"
    );
    assert!(
        ProductRepo::list_for_owner(&pool, u)
            .await
            .unwrap()
            .is_empty(),
        "hidden from favorites"
    );

    // Repeat, with other casing and another category chosen: same product,
    // category unchanged.
    let again = RecipeRepo::add_ingredient(&pool, u, puuro, &ingredient("MAITO", None, Some(hevi)))
        .await
        .unwrap();
    assert_eq!(again.product_id, Some(product.id));
    assert_eq!(again.text, "Maito", "shows the product's name");
    assert_eq!(again.category.map(|c| c.id), Some(kylma));
    let reread = ProductRepo::find_by_name(&pool, u, "Maito")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reread.category_id, Some(kylma));

    // An existing favorite is reused and stays a favorite.
    let kahvi = ProductRepo::create(&pool, u, "Kahvi", hevi).await.unwrap();
    let linked = RecipeRepo::add_ingredient(&pool, u, puuro, &ingredient("kahvi", None, None))
        .await
        .unwrap();
    assert_eq!(linked.product_id, Some(kahvi.id));
    assert!(
        ProductRepo::find_by_name(&pool, u, "Kahvi")
            .await
            .unwrap()
            .unwrap()
            .is_favorite
    );

    // A new name needs a category of the user's own; nothing is created
    // otherwise.
    for category in [None, Some(theirs), Some(Uuid::new_v4())] {
        assert!(matches!(
            RecipeRepo::add_ingredient(&pool, u, puuro, &ingredient("Kaura", None, category)).await,
            Err(CoreError::Invalid(_))
        ));
    }
    assert!(
        ProductRepo::find_by_name(&pool, u, "Kaura")
            .await
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        RecipeRepo::add_ingredient(&pool, u, puuro, &ingredient("  ", None, Some(kylma))).await,
        Err(CoreError::Invalid(_))
    ));
    assert_eq!(
        names(&RecipeRepo::get(&pool, u, puuro).await.unwrap()),
        ["Maito", "Kahvi"]
    );

    // Another user's same name is their own product.
    let theirs_recipe = make_recipe(&pool, other, "Smoothie").await;
    let their_milk = RecipeRepo::add_ingredient(
        &pool,
        other,
        theirs_recipe,
        &ingredient("maito", None, Some(theirs)),
    )
    .await
    .unwrap();
    assert_ne!(their_milk.product_id, Some(product.id));
}

/// P4: quantity is optional and round-trips unchanged (blank means none).
#[tokio::test]
async fn ingredient_quantity_roundtrip() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let mausteet = make_category(&pool, u, "Mausteet").await;
    let recipe = make_recipe(&pool, u, "Pulla").await;

    let cases = [
        ("Kardemumma", Some("1 rkl"), Some("1 rkl")),
        ("Sokeri", Some("1/2 tl"), Some("1/2 tl")),
        ("Suola", Some("ripaus "), Some("ripaus")),
        ("Kaneli", None, None),
        ("Vanilja", Some("   "), None),
    ];
    for (name, quantity, _) in cases {
        let added = RecipeRepo::add_ingredient(
            &pool,
            u,
            recipe,
            &ingredient(name, quantity, Some(mausteet)),
        )
        .await
        .unwrap();
        assert_eq!(added.text, name);
    }
    let detail = RecipeRepo::get(&pool, u, recipe).await.unwrap();
    let stored: Vec<_> = detail
        .ingredients
        .iter()
        .map(|i| (i.text.as_str(), i.quantity.as_deref()))
        .collect();
    let expected: Vec<_> = cases.iter().map(|&(n, _, q)| (n, q)).collect();
    assert_eq!(stored, expected);

    // Setting and clearing a quantity later.
    let kaneli = detail.ingredients[3].id;
    RecipeRepo::set_ingredient_quantity(&pool, u, recipe, kaneli, Some("2 tl"))
        .await
        .unwrap();
    let quantity = |d: &RecipeDetail| d.ingredients[3].quantity.clone();
    assert_eq!(
        quantity(&RecipeRepo::get(&pool, u, recipe).await.unwrap()).as_deref(),
        Some("2 tl")
    );
    RecipeRepo::set_ingredient_quantity(&pool, u, recipe, kaneli, Some(""))
        .await
        .unwrap();
    assert_eq!(
        quantity(&RecipeRepo::get(&pool, u, recipe).await.unwrap()),
        None
    );
}

/// P5: ingredients come back in their stored position order, which moves and
/// removals keep consistent.
#[tokio::test]
async fn ingredients_ordered() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let recipe = make_recipe(&pool, u, "Salaatti").await;

    let mut ids = Vec::new();
    for name in ["Tomaatti", "Kurkku", "Salaatti", "Retiisi"] {
        ids.push(add(&pool, u, recipe, ingredient(name, None, Some(hevi))).await);
    }
    let order = || async { names(&RecipeRepo::get(&pool, u, recipe).await.unwrap()).join(",") };
    assert_eq!(order().await, "Tomaatti,Kurkku,Salaatti,Retiisi");

    RecipeRepo::move_ingredient(&pool, u, recipe, ids[3], Direction::Up)
        .await
        .unwrap();
    assert_eq!(order().await, "Tomaatti,Kurkku,Retiisi,Salaatti");
    RecipeRepo::move_ingredient(&pool, u, recipe, ids[0], Direction::Down)
        .await
        .unwrap();
    assert_eq!(order().await, "Kurkku,Tomaatti,Retiisi,Salaatti");

    // Past either end: no-op.
    RecipeRepo::move_ingredient(&pool, u, recipe, ids[1], Direction::Up)
        .await
        .unwrap();
    RecipeRepo::move_ingredient(&pool, u, recipe, ids[2], Direction::Down)
        .await
        .unwrap();
    assert_eq!(order().await, "Kurkku,Tomaatti,Retiisi,Salaatti");

    // Removing leaves the rest in order; a new ingredient goes last.
    RecipeRepo::remove_ingredient(&pool, u, recipe, ids[0])
        .await
        .unwrap();
    assert_eq!(order().await, "Kurkku,Retiisi,Salaatti");
    add(&pool, u, recipe, ingredient("Sipuli", None, Some(hevi))).await;
    assert_eq!(order().await, "Kurkku,Retiisi,Salaatti,Sipuli");
    let positions: Vec<i64> = RecipeRepo::get(&pool, u, recipe)
        .await
        .unwrap()
        .ingredients
        .iter()
        .map(|i| i.position)
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
}

/// Another user's recipe, and an ingredient outside the given recipe, are
/// `NotFound` for every method, and nothing is changed.
#[tokio::test]
async fn owner_scoped() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let theirs_hevi = make_category(&pool, other, "Hevi").await;
    let mine = make_recipe(&pool, u, "Keitto").await;
    let also_mine = make_recipe(&pool, u, "Salaatti").await;
    let theirs = make_recipe(&pool, other, "Piirakka").await;
    let carrot = add(&pool, u, mine, ingredient("Porkkana", None, Some(hevi))).await;
    let their_apple = add(
        &pool,
        other,
        theirs,
        ingredient("Omena", None, Some(theirs_hevi)),
    )
    .await;

    let not_found = |r: Result<_, CoreError>| matches!(r, Err(CoreError::NotFound));
    let input = RecipeInput {
        title: "Kaapattu".to_owned(),
        ..RecipeInput::default()
    };
    assert!(not_found(
        RecipeRepo::get(&pool, u, theirs).await.map(|_| ())
    ));
    assert!(not_found(
        RecipeRepo::update(&pool, u, theirs, &input)
            .await
            .map(|_| ())
    ));
    assert!(not_found(
        RecipeRepo::add_ingredient(&pool, u, theirs, &ingredient("Omena", None, Some(hevi)))
            .await
            .map(|_| ())
    ));
    for (recipe, item) in [
        (theirs, their_apple),
        (also_mine, carrot),
        (mine, their_apple),
    ] {
        assert!(not_found(
            RecipeRepo::set_ingredient_quantity(&pool, u, recipe, item, Some("1")).await
        ));
        assert!(not_found(
            RecipeRepo::move_ingredient(&pool, u, recipe, item, Direction::Up).await
        ));
        assert!(not_found(
            RecipeRepo::remove_ingredient(&pool, u, recipe, item).await
        ));
    }

    let detail = RecipeRepo::get(&pool, other, theirs).await.unwrap();
    assert_eq!(detail.recipe.title, "Piirakka");
    assert_eq!(names(&detail), ["Omena"]);
    assert_eq!(detail.ingredients[0].quantity, None);
    assert_eq!(
        names(&RecipeRepo::get(&pool, u, mine).await.unwrap()),
        ["Porkkana"]
    );
    assert!(
        ProductRepo::find_by_name(&pool, u, "Omena")
            .await
            .unwrap()
            .is_none()
    );
}

/// Deleting a product unlinks its ingredients instead of deleting them
/// (RR-0001 constraint 4); they keep the name they were saved with.
#[tokio::test]
async fn deleted_product_keeps_ingredient() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let recipe = make_recipe(&pool, u, "Pannukakku").await;
    add(
        &pool,
        u,
        recipe,
        ingredient("Maito", Some("5 dl"), Some(kylma)),
    )
    .await;
    let product = ProductRepo::find_by_name(&pool, u, "Maito")
        .await
        .unwrap()
        .unwrap();

    ProductRepo::delete(&pool, u, product.id).await.unwrap();

    let detail = RecipeRepo::get(&pool, u, recipe).await.unwrap();
    assert_eq!(names(&detail), ["Maito"]);
    let unlinked = &detail.ingredients[0];
    assert_eq!(unlinked.product_id, None);
    assert!(unlinked.category.is_none());
    assert_eq!(unlinked.quantity.as_deref(), Some("5 dl"));
}
