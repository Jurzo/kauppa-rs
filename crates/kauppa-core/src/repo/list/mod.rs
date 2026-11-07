//! MB-0007 acceptance tests for the shopping-list repository (P1, P2, P4) and
//! MB-0004's for list items (P1–P4), MB-0005's for adding a recipe (P1–P4),
//! and MB-0006's store-view order (P2).
//! Test-only module: `ShoppingListRepo` lives in `repo` and its SQLite impl in
//! `repo::sqlite`; this module exists so the brief's
//! `cargo test -p kauppa-core repo::list::<area>::<name>` checks select these tests.
//!
//! Split by area (RR-0009): `lifecycle` (MB-0007), `items` (MB-0004),
//! `recipes` (MB-0005) and `store` (MB-0006). Fixtures shared by several
//! areas live here.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod items;
mod lifecycle;
mod recipes;
mod store;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use uuid::Uuid;

use crate::repo::{CategoryRepo, ShoppingListRepo, SqlitePool, UserRepo};

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

fn item(text: &str, category: Option<&str>) -> (String, Option<String>) {
    (text.to_owned(), category.map(str::to_owned))
}
