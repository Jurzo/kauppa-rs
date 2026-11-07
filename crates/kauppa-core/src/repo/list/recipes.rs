//! MB-0005 (P1–P4): adding a recipe to a list.

use uuid::Uuid;

use crate::CoreError;
use crate::domain::{NewIngredient, RecipeInput};
use crate::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo, SqlitePool};

use super::{make_category, make_list, make_user, test_db};

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
