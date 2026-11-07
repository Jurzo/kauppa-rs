//! Recipe-created (hidden) products: MB-0003's favorites rules and MB-0014 (P1, P2).

use uuid::Uuid;

use crate::CoreError;
use crate::domain::{Direction, NewIngredient, RecipeInput};
use crate::repo::{CategoryRepo, ProductRepo, RecipeRepo, SqlitePool};

use super::{make_category, make_hidden, make_user, shape, test_pool};

/// MB-0003 (owner decision): favorites views list only favorites; products
/// created from recipe ingredients stay hidden.
#[tokio::test]
async fn list_hides_non_favorites() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;

    ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    make_hidden(&pool, u, "Kurkku", hevi).await;
    make_hidden(&pool, u, "Maito", kylma).await;

    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Hevi"), vec!["Omena"])]
    );
}

/// MB-0003 (owner decision): adding a favorite whose name matches a hidden
/// product promotes that product — same row, moved to the chosen category —
/// instead of answering "already exists". An existing favorite still
/// conflicts.
#[tokio::test]
async fn create_promotes_hidden_product() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let theirs = make_category(&pool, other, "Kylmä").await;
    ProductRepo::create(&pool, u, "Jogurtti", kylma)
        .await
        .unwrap();
    let hidden = make_hidden(&pool, u, "maito", hevi).await;

    // A foreign category is refused before anything is promoted.
    assert!(matches!(
        ProductRepo::create(&pool, u, "Maito", theirs).await,
        Err(CoreError::Invalid(_))
    ));
    assert!(
        !ProductRepo::find_by_name(&pool, u, "maito")
            .await
            .unwrap()
            .unwrap()
            .is_favorite
    );

    let promoted = ProductRepo::create(&pool, u, "MAITO", kylma).await.unwrap();
    assert_eq!(
        promoted.id, hidden,
        "the same row, so recipes keep pointing at it"
    );
    assert!(promoted.is_favorite);
    assert_eq!(promoted.name, "maito", "keeps its stored name");
    assert_eq!(promoted.category_id, Some(kylma));
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Kylmä"), vec!["Jogurtti", "maito"])],
        "appended at the end of the chosen category"
    );

    assert!(matches!(
        ProductRepo::create(&pool, u, "Maito", kylma).await,
        Err(CoreError::Conflict(_))
    ));
}

/// MB-0003: moving a favorite steps past the next favorite; hidden products in
/// the same category keep their slots and never block a move.
#[tokio::test]
async fn move_skips_hidden_products() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;

    let omena = ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    make_hidden(&pool, u, "Kurkku", hevi).await;
    let porkkana = ProductRepo::create(&pool, u, "Porkkana", hevi)
        .await
        .unwrap();

    ProductRepo::move_within_category(&pool, u, porkkana.id, Direction::Up)
        .await
        .unwrap();
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Hevi"), vec!["Porkkana", "Omena"])]
    );
    // Omena is now the last favorite, so down is a no-op even though the
    // hidden Kurkku sits between the two in position order.
    ProductRepo::move_within_category(&pool, u, omena.id, Direction::Down)
        .await
        .unwrap();
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Hevi"), vec!["Porkkana", "Omena"])]
    );
}

/// A recipe titled `title` with one ingredient per name in `names` (new
/// products go to `category`); returns the recipe id.
async fn make_recipe(
    pool: &SqlitePool,
    owner: Uuid,
    title: &str,
    names: &[&str],
    category: Uuid,
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
    for name in names {
        let new = NewIngredient {
            product_name: (*name).to_owned(),
            quantity: None,
            category_id: Some(category),
        };
        RecipeRepo::add_ingredient(pool, owner, recipe, &new)
            .await
            .unwrap();
    }
    recipe
}

/// `(group name, [(product name, recipe count)])` per group of
/// `list_hidden_with_usage`; `None` is uncategorized.
async fn hidden_shape(pool: &SqlitePool, owner: Uuid) -> Vec<(Option<String>, Vec<(String, i64)>)> {
    ProductRepo::list_hidden_with_usage(pool, owner)
        .await
        .unwrap()
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name),
                g.products
                    .into_iter()
                    .map(|u| (u.product.name, u.recipe_count))
                    .collect(),
            )
        })
        .collect()
}

/// MB-0014 P1: the owner's recipe-created products, grouped in category order
/// with uncategorized last and ordered by name (ignoring case) within a group,
/// each with the number of the owner's recipes that use it. Favorites and
/// another user's products never appear, and another user's recipes never
/// count.
#[tokio::test]
async fn list_hidden_with_usage() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let juomat = make_category(&pool, u, "Juomat").await;
    let theirs = make_category(&pool, other, "Hevi").await;

    assert!(
        ProductRepo::list_hidden_with_usage(&pool, u)
            .await
            .unwrap()
            .is_empty()
    );

    // A favorite, also used by a recipe: never listed here.
    ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    // Kurkku: in two recipes, twice in the first — counted once per recipe.
    make_recipe(
        &pool,
        u,
        "Salaatti",
        &["Tomaatti", "kurkku", "Kurkku", "Omena"],
        hevi,
    )
    .await;
    make_recipe(&pool, u, "Tzatziki", &["Kurkku", "Jogurtti"], kylma).await;
    // Unused: its only ingredient is removed again.
    let scratch = make_recipe(&pool, u, "Luonnos", &["Kerma"], kylma).await;
    let kerma = RecipeRepo::get(&pool, u, scratch)
        .await
        .unwrap()
        .ingredients[0]
        .id;
    RecipeRepo::remove_ingredient(&pool, u, scratch, kerma)
        .await
        .unwrap();
    // Uncategorized after its category is deleted.
    make_recipe(&pool, u, "Aamiainen", &["Mehu"], juomat).await;
    CategoryRepo::delete(&pool, u, juomat).await.unwrap();

    // The other user: same names, more recipes, never mixed in.
    for title in ["A", "B", "C"] {
        make_recipe(&pool, other, title, &["Kurkku", "Mehu"], theirs).await;
    }

    let row = |name: &str, count: i64| (name.to_owned(), count);
    assert_eq!(
        hidden_shape(&pool, u).await,
        [
            (
                Some("Hevi".to_owned()),
                vec![row("kurkku", 2), row("Tomaatti", 1)]
            ),
            (
                Some("Kylmä".to_owned()),
                vec![row("Jogurtti", 1), row("Kerma", 0)]
            ),
            (None, vec![row("Mehu", 1)]),
        ]
    );
    assert_eq!(
        hidden_shape(&pool, other).await,
        [(
            Some("Hevi".to_owned()),
            vec![row("Kurkku", 3), row("Mehu", 3)]
        )]
    );
    // The favorites list is unchanged.
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Hevi"), vec!["Omena"])]
    );
}

/// MB-0014 P2: making a recipe-created product a favorite keeps its id, name,
/// and category and appends it after that category's favorites; an
/// uncategorized one is refused (`Invalid`) until it has a category. Another
/// user's product is `NotFound`, and a favorite is left as it is.
#[tokio::test]
async fn make_favorite() {
    let pool = test_pool().await;
    let u = make_user(&pool, "u@example.com").await;
    let other = make_user(&pool, "other@example.com").await;
    let hevi = make_category(&pool, u, "Hevi").await;
    let kylma = make_category(&pool, u, "Kylmä").await;
    let juomat = make_category(&pool, u, "Juomat").await;

    let kurkku = make_hidden(&pool, u, "kurkku", hevi).await;
    ProductRepo::create(&pool, u, "Omena", hevi).await.unwrap();
    let porkkana = ProductRepo::create(&pool, u, "Porkkana", hevi)
        .await
        .unwrap();
    let mehu = make_hidden(&pool, u, "Mehu", juomat).await;
    CategoryRepo::delete(&pool, u, juomat).await.unwrap();

    // Another user can't promote it, and nothing changes.
    assert!(matches!(
        ProductRepo::make_favorite(&pool, other, kurkku).await,
        Err(CoreError::NotFound)
    ));
    assert!(matches!(
        ProductRepo::make_favorite(&pool, u, Uuid::new_v4()).await,
        Err(CoreError::NotFound)
    ));
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Hevi"), vec!["Omena", "Porkkana"])]
    );

    let favorite = ProductRepo::make_favorite(&pool, u, kurkku).await.unwrap();
    assert_eq!(favorite.id, kurkku, "the same row");
    assert_eq!(favorite.name, "kurkku", "keeps its name");
    assert_eq!(favorite.category_id, Some(hevi), "keeps its category");
    assert!(favorite.is_favorite);
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Hevi"), vec!["Omena", "Porkkana", "kurkku"])],
        "appended at the end of its category's favorites"
    );
    // It moves among the favorites like any other.
    ProductRepo::move_within_category(&pool, u, porkkana.id, Direction::Down)
        .await
        .unwrap();
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Hevi"), vec!["Omena", "kurkku", "Porkkana"])]
    );

    // Already a favorite: a no-op that keeps its place.
    let again = ProductRepo::make_favorite(&pool, u, kurkku).await.unwrap();
    assert!(again.is_favorite);
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [(Some("Hevi"), vec!["Omena", "kurkku", "Porkkana"])]
    );

    // Uncategorized: refused until a category is set.
    assert!(matches!(
        ProductRepo::make_favorite(&pool, u, mehu).await,
        Err(CoreError::Invalid(_))
    ));
    assert!(
        !ProductRepo::find_by_name(&pool, u, "Mehu")
            .await
            .unwrap()
            .unwrap()
            .is_favorite
    );
    ProductRepo::recategorize(&pool, u, mehu, kylma)
        .await
        .unwrap();
    let mehu = ProductRepo::make_favorite(&pool, u, mehu).await.unwrap();
    assert_eq!(mehu.category_id, Some(kylma));
    assert_eq!(
        shape(&ProductRepo::list_for_owner(&pool, u).await.unwrap()),
        [
            (Some("Hevi"), vec!["Omena", "kurkku", "Porkkana"]),
            (Some("Kylmä"), vec!["Mehu"]),
        ]
    );
    assert!(
        ProductRepo::list_hidden_with_usage(&pool, u)
            .await
            .unwrap()
            .is_empty(),
        "nothing hidden is left"
    );
}
