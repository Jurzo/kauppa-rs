//! Tests for `routes::lists`, by area (RR-0009). Fixtures shared by several areas live here.

mod add_field;
mod editor_match;
mod item_edit;
mod items;
mod lifecycle;
mod recipes;
mod sharing_invites;
mod sharing_roles;
mod shop;

use uuid::Uuid;

use kauppa_core::domain::{ListItem, ShareRole};
use kauppa_core::repo::{ProductRepo, ShoppingListRepo};

#[cfg(test)]
use crate::routes::test_support::make_recipe_with;
use crate::state::AppState;

/// A user with a list for the store-view tests: Kylmä (Maito, Jogurtti),
/// Hevi (Omena), an empty Pakasteet, a free-text "Kynttilät", and a removed
/// "Sokeri".
struct ShopFixture {
    list: Uuid,
    milk: Uuid,
    candles: Uuid,
    sugar: Uuid,
}

/// An owner's list shared with a `view` and an `edit` grantee, plus the
/// owner's catalog: one active item (`bread`), one removed (`sugar`).
#[derive(Debug, Clone, Copy)]
struct Shared {
    owner: Uuid,
    viewer: Uuid,
    editor: Uuid,
    list: Uuid,
    bread: Uuid,
    sugar: Uuid,
    category: Uuid,
    product: Uuid,
    recipe: Uuid,
    viewer_share: Uuid,
    editor_share: Uuid,
}

#[allow(clippy::expect_used)]
async fn shared_list(state: &AppState) -> Shared {
    use crate::routes::test_support::{make_category, make_user};

    let owner = make_user(state, "owner@example.com").await;
    let viewer = make_user(state, "viewer@example.com").await;
    let editor = make_user(state, "editor@example.com").await;
    let category = make_category(state, owner, "Kylmä").await;
    let product = ProductRepo::create(&state.pool, owner, "Maito", category)
        .await
        .expect("favorite")
        .id;
    let recipe =
        make_recipe_with(state, owner, "Puuro", category, &[("Kaura", Some("2 dl"))]).await;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    let bread = ShoppingListRepo::add_item(&state.pool, owner, list, "Leipä", None)
        .await
        .expect("item")
        .id;
    let sugar = ShoppingListRepo::add_item(&state.pool, owner, list, "Sokeri", None)
        .await
        .expect("item")
        .id;
    ShoppingListRepo::remove_item(&state.pool, owner, list, sugar)
        .await
        .expect("remove");
    let viewer_share = ShoppingListRepo::share(
        &state.pool,
        owner,
        list,
        "viewer@example.com",
        ShareRole::View,
    )
    .await
    .expect("share")
    .expect("viewer matched")
    .id;
    let editor_share = ShoppingListRepo::share(
        &state.pool,
        owner,
        list,
        "editor@example.com",
        ShareRole::Edit,
    )
    .await
    .expect("share")
    .expect("editor matched")
    .id;
    Shared {
        owner,
        viewer,
        editor,
        list,
        bread,
        sugar,
        category,
        product,
        recipe,
        viewer_share,
        editor_share,
    }
}

/// One POST on a list: `(name, uri, form)`.
type ListPost = (&'static str, String, Vec<(&'static str, String)>);

/// Every POST on a list, as `(name, uri, form)`.
fn list_posts(s: &Shared) -> Vec<ListPost> {
    let base = format!("/lists/{}", s.list);
    let (bread, sugar) = (s.bread, s.sugar);
    vec![
        (
            "add_item",
            format!("{base}/items"),
            vec![("text", "Munat".to_owned())],
        ),
        (
            "add_recipe",
            format!("{base}/recipes"),
            vec![
                ("recipe_id", s.recipe.to_string()),
                ("multiplier", "1".to_owned()),
            ],
        ),
        (
            "check",
            format!("{base}/shop/items/{bread}/check"),
            vec![("checked", "true".to_owned())],
        ),
        (
            "shop_remove",
            format!("{base}/shop/items/{bread}/remove"),
            vec![],
        ),
        (
            // Saving an item with a category is the owner's (RR-0033).
            "category",
            format!("{base}/items/{bread}"),
            vec![
                ("category_id", s.category.to_string()),
                ("quantity", "2".to_owned()),
            ],
        ),
        ("remove", format!("{base}/items/{bread}/remove"), vec![]),
        ("restore", format!("{base}/items/{sugar}/restore"), vec![]),
        (
            "rename",
            format!("{base}/rename"),
            vec![("name", "Hijacked".to_owned())],
        ),
        ("delete", format!("{base}/delete"), vec![]),
        (
            "share",
            format!("{base}/shares"),
            vec![
                ("email", "x@example.com".to_owned()),
                ("role", "edit".to_owned()),
            ],
        ),
        (
            "revoke",
            format!("{base}/shares/{}/revoke", s.viewer_share),
            vec![],
        ),
    ]
}

/// Everything about a list that a mutation could change, read as the owner.
#[allow(clippy::expect_used)]
async fn list_snapshot(state: &AppState, s: &Shared) -> String {
    let list = ShoppingListRepo::get(&state.pool, s.owner, s.list)
        .await
        .expect("list");
    let row = |i: ListItem| (i.text, i.checked, i.category.map(|c| c.name));
    let active: Vec<_> = ShoppingListRepo::items(&state.pool, s.owner, s.list, false)
        .await
        .expect("items")
        .into_iter()
        .map(row)
        .collect();
    let removed: Vec<_> = ShoppingListRepo::items(&state.pool, s.owner, s.list, true)
        .await
        .expect("items")
        .into_iter()
        .map(row)
        .collect();
    let shares: Vec<_> = ShoppingListRepo::shares(&state.pool, s.owner, s.list)
        .await
        .expect("shares")
        .into_iter()
        .map(|g| (g.grantee_id, g.role))
        .collect();
    format!("{} {active:?} {removed:?} {shares:?}", list.name)
}

/// `(text, category)` of a list's active items, in order.
#[allow(clippy::expect_used)]
async fn item_shape(state: &AppState, user: Uuid, list: Uuid) -> Vec<(String, Option<String>)> {
    ShoppingListRepo::items(&state.pool, user, list, false)
        .await
        .expect("list items")
        .into_iter()
        .map(|i| (i.text, i.category.map(|c| c.name)))
        .collect()
}
