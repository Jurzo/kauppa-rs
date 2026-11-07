//! MB-0002 acceptance tests for the product repository (P2–P5), plus the
//! repo-level half of P1, MB-0010 P2 and P3, MB-0003's favorites rules,
//! MB-0013 P1, and MB-0014 P1–P2. Test-only module: `ProductRepo` lives in `repo` and
//! its SQLite impl in `repo::sqlite`; this module exists so the brief's
//! `cargo test -p kauppa-core repo::product::<area>::<name>` checks select these
//! tests.
//!
//! Split by area (RR-0009): `catalog` (MB-0002), `names` (MB-0010),
//! `hidden` (MB-0003's favorites rules and MB-0014) and `suggestions`
//! (MB-0013). Fixtures shared by several areas live here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod catalog;
mod hidden;
mod names;
mod suggestions;

use uuid::Uuid;

use crate::domain::{NewIngredient, ProductGroup, RecipeInput};
use crate::repo::{CategoryRepo, RecipeRepo, SqlitePool, UserRepo};

/// A fresh, isolated in-memory database with migrations applied.
async fn test_pool() -> SqlitePool {
    let url = format!(
        "sqlite:file:product-{}?mode=memory&cache=shared",
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

/// `(group name, [product names])` per group; `None` is uncategorized.
fn shape(groups: &[ProductGroup]) -> Vec<(Option<&str>, Vec<&str>)> {
    groups
        .iter()
        .map(|g| {
            (
                g.category.as_ref().map(|c| c.name.as_str()),
                g.products.iter().map(|p| p.name.as_str()).collect(),
            )
        })
        .collect()
}

/// A recipe-created product (`is_favorite = 0`, MB-0003) for these tests.
async fn make_hidden(pool: &SqlitePool, owner: Uuid, name: &str, category: Uuid) -> Uuid {
    let recipe = RecipeRepo::create(
        pool,
        owner,
        &RecipeInput {
            title: format!("Recipe with {name}"),
            ..RecipeInput::default()
        },
    )
    .await
    .unwrap()
    .id;
    let new = NewIngredient {
        product_name: name.to_owned(),
        quantity: None,
        category_id: Some(category),
    };
    RecipeRepo::add_ingredient(pool, owner, recipe, &new)
        .await
        .unwrap()
        .product_id
        .expect("linked product")
}
