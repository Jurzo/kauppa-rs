//! MB-0036 follow-up (RR-0024): `add_owner_favorite_by_name` — typed text
//! that names one of the list owner's favorites adds that product, at `edit`.

use uuid::Uuid;

use crate::CoreError;
use crate::domain::{NewIngredient, RecipeInput, ShareRole};
use crate::repo::{ProductRepo, RecipeRepo, ShoppingListRepo, SqlitePool};

use super::{make_category, make_list, make_user, test_db};

/// `(text, quantity, category name, product id)` of the active items.
type Row = (String, Option<String>, Option<String>, Option<Uuid>);

async fn rows(pool: &SqlitePool, owner: Uuid, list: Uuid) -> Vec<Row> {
    ShoppingListRepo::items(pool, owner, list, false)
        .await
        .unwrap()
        .into_iter()
        .map(|i| (i.text, i.quantity, i.category.map(|c| c.name), i.product_id))
        .collect()
}

fn row(text: &str, quantity: Option<&str>, category: Option<&str>, product: Option<Uuid>) -> Row {
    (
        text.to_owned(),
        quantity.map(str::to_owned),
        category.map(str::to_owned),
        product,
    )
}

/// RR-0024 sub-answer 1 and constraints 2–4: an `edit` grantee's text
/// matches only the list owner's favorites by `name_key` (the owner's
/// product, name and category); the editor's own favorites and the owner's
/// non-favorite (recipe-created) products stay free text; no match is a
/// free-text item. A `view` grantee gets `Forbidden` and a stranger
/// `NotFound`, writing nothing; a revoked editor is denied at once.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn editor_matches_only_owner_favorites() {
    let (pool, raw) = test_db().await;
    let owner = make_user(&pool, "owner@example.com").await;
    let editor = make_user(&pool, "editor@example.com").await;
    let viewer = make_user(&pool, "viewer@example.com").await;
    let stranger = make_user(&pool, "stranger@example.com").await;

    let kylma = make_category(&pool, owner, "Kylmä").await;
    let maito = ProductRepo::create(&pool, owner, "Maito", kylma)
        .await
        .unwrap()
        .id;
    // A recipe-created product of the owner's: categorized, not a favorite.
    let recipe = RecipeRepo::create(
        &pool,
        owner,
        &RecipeInput {
            title: "Puuro".to_owned(),
            ..RecipeInput::default()
        },
    )
    .await
    .unwrap()
    .id;
    let kaura = NewIngredient {
        product_name: "Kaura".to_owned(),
        quantity: None,
        category_id: Some(kylma),
    };
    RecipeRepo::add_ingredient(&pool, owner, recipe, &kaura)
        .await
        .unwrap();
    // The editor's own favorites: one name the owner lacks, one they share.
    let hevi = make_category(&pool, editor, "Hevi").await;
    ProductRepo::create(&pool, editor, "Omena", hevi)
        .await
        .unwrap();
    let their_maito = make_category(&pool, editor, "Maitotuotteet").await;
    ProductRepo::create(&pool, editor, "Maito", their_maito)
        .await
        .unwrap();

    let list = make_list(&pool, owner).await;
    let share = ShoppingListRepo::share(&pool, owner, list, "editor@example.com", ShareRole::Edit)
        .await
        .unwrap()
        .unwrap();
    ShoppingListRepo::share(&pool, owner, list, "viewer@example.com", ShareRole::View)
        .await
        .unwrap();

    let add = |user: Uuid, text: &'static str, quantity: Option<&'static str>| {
        let pool = &pool;
        async move {
            ShoppingListRepo::add_owner_favorite_by_name(pool, user, list, text, quantity).await
        }
    };

    // A match: the owner's product, its name and category; quantity kept.
    let item = add(editor, "  mAITO ", Some("2 l")).await.unwrap();
    assert_eq!(item.product_id, Some(maito));
    assert_eq!(item.text, "Maito");
    assert_eq!(item.category.map(|c| c.id), Some(kylma));
    // No match: the editor's own favorite, the owner's non-favorite, and
    // anything else are free text, as typed.
    add(editor, "omena", None).await.unwrap();
    add(editor, "kaura", Some("1 kg")).await.unwrap();
    add(editor, "Kynttilät", None).await.unwrap();
    // The owner's own text matches the same way.
    add(owner, "maito", None).await.unwrap();
    assert_eq!(
        rows(&pool, owner, list).await,
        [
            row("Maito", Some("2 l"), Some("Kylmä"), Some(maito)),
            row("omena", None, None, None),
            row("kaura", Some("1 kg"), None, None),
            row("Kynttilät", None, None, None),
            row("Maito", None, Some("Kylmä"), Some(maito)),
        ]
    );

    // Empty text is rejected, as `add_item` rejects it.
    assert!(matches!(
        add(editor, "  ", None).await,
        Err(CoreError::Invalid(_))
    ));

    // A viewer and a stranger are denied, and nothing is written.
    let count = || async {
        sqlx::query_scalar::<_, i64>(r"SELECT COUNT(*) FROM list_items")
            .fetch_one(&raw)
            .await
            .unwrap()
    };
    let before = count().await;
    assert!(matches!(
        add(viewer, "maito", None).await,
        Err(CoreError::Forbidden)
    ));
    assert!(matches!(
        add(stranger, "maito", None).await,
        Err(CoreError::NotFound)
    ));
    // Revocation is immediate (RR-0002 constraint 4).
    ShoppingListRepo::revoke(&pool, owner, list, share.id)
        .await
        .unwrap();
    assert!(matches!(
        add(editor, "maito", None).await,
        Err(CoreError::NotFound)
    ));
    assert_eq!(count().await, before, "denied calls wrote nothing");
}

/// RR-0024 constraint 4: the id-based add stays the owner's, even for the
/// owner's favorite; only typed text matches at `edit`.
#[tokio::test]
async fn add_product_stays_owner_only() {
    let (pool, _raw) = test_db().await;
    let owner = make_user(&pool, "owner@example.com").await;
    let editor = make_user(&pool, "editor@example.com").await;
    let kylma = make_category(&pool, owner, "Kylmä").await;
    let maito = ProductRepo::create(&pool, owner, "Maito", kylma)
        .await
        .unwrap()
        .id;
    let list = make_list(&pool, owner).await;
    ShoppingListRepo::share(&pool, owner, list, "editor@example.com", ShareRole::Edit)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        ShoppingListRepo::add_product(&pool, editor, list, maito, None).await,
        Err(CoreError::Forbidden)
    ));
    let item = ShoppingListRepo::add_owner_favorite_by_name(&pool, editor, list, "Maito", None)
        .await
        .unwrap();
    assert_eq!(item.product_id, Some(maito));
}
