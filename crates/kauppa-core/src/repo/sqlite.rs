//! SQLite-backed persistence.
//!
//! Uses runtime-checked `sqlx::query` so the scaffold builds without
//! `DATABASE_URL`. Once the schema is stable, switch to the `query!` macros +
//! `cargo sqlx prepare` offline cache for compile-time checking — tracked in
//! `.agentic/mentorship/persistence.md`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{Recipe, ShoppingList, User};
use crate::error::CoreError;
use crate::repo::{RecipeRepo, ShoppingListRepo, UserRepo};

#[derive(Clone, Debug)]
pub struct SqlitePool {
    inner: sqlx::SqlitePool,
}

impl SqlitePool {
    pub async fn connect(url: &str) -> CoreResult<Self> {
        let options = SqliteConnectOptions::from_str(url)
            .map_err(|e| CoreError::Invalid(format!("bad database url: {e}")))?
            .create_if_missing(true)
            .foreign_keys(true);

        let inner = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;

        Ok(Self { inner })
    }

    pub async fn run_migrations(&self) -> CoreResult<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.inner)
            .await
            .map_err(|e| CoreError::Invalid(format!("migration failed: {e}")))?;
        Ok(())
    }
}

fn parse_uuid(s: &str) -> CoreResult<Uuid> {
    Uuid::parse_str(s).map_err(|e| CoreError::Invalid(format!("invalid uuid: {e}")))
}

#[async_trait]
impl UserRepo for SqlitePool {
    async fn find_by_email(&self, email: &str) -> CoreResult<Option<(User, String)>> {
        let row = sqlx::query(
            r"SELECT id, email, display_name, password_hash, created_at
                FROM users
                WHERE email = ?1",
        )
        .bind(email)
        .fetch_optional(&self.inner)
        .await?;

        let Some(row) = row else { return Ok(None) };

        let id_str: String = row.try_get("id")?;
        let email: String = row.try_get("email")?;
        let display_name: String = row.try_get("display_name")?;
        let password_hash: String = row.try_get("password_hash")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;

        Ok(Some((
            User {
                id: parse_uuid(&id_str)?,
                email,
                display_name,
                created_at,
            },
            password_hash,
        )))
    }

    async fn create(
        &self,
        email: &str,
        display_name: &str,
        password_hash: &str,
    ) -> CoreResult<User> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let now = Utc::now();

        sqlx::query(
            r"INSERT INTO users (id, email, display_name, password_hash, created_at)
              VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&id_str)
        .bind(email)
        .bind(display_name)
        .bind(password_hash)
        .bind(now)
        .execute(&self.inner)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                CoreError::Conflict(format!("email already registered: {email}"))
            }
            _ => CoreError::Database(e),
        })?;

        Ok(User {
            id,
            email: email.to_owned(),
            display_name: display_name.to_owned(),
            created_at: now,
        })
    }
}

#[async_trait]
impl ShoppingListRepo for SqlitePool {
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ShoppingList>> {
        let owner = owner_id.to_string();
        let rows = sqlx::query(
            r"SELECT id, owner_id, name, created_at, updated_at
                FROM shopping_lists
                WHERE owner_id = ?1
                ORDER BY updated_at DESC",
        )
        .bind(&owner)
        .fetch_all(&self.inner)
        .await?;

        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                let owner_id: String = row.try_get("owner_id")?;
                Ok(ShoppingList {
                    id: parse_uuid(&id)?,
                    owner_id: parse_uuid(&owner_id)?,
                    name: row.try_get("name")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect()
    }
}

#[async_trait]
impl RecipeRepo for SqlitePool {
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Recipe>> {
        let owner = owner_id.to_string();
        let rows = sqlx::query(
            r"SELECT id, owner_id, title, instructions, servings, created_at, updated_at
                FROM recipes
                WHERE owner_id = ?1
                ORDER BY updated_at DESC",
        )
        .bind(&owner)
        .fetch_all(&self.inner)
        .await?;

        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                let owner_id: String = row.try_get("owner_id")?;
                Ok(Recipe {
                    id: parse_uuid(&id)?,
                    owner_id: parse_uuid(&owner_id)?,
                    title: row.try_get("title")?,
                    instructions: row.try_get("instructions")?,
                    servings: row.try_get("servings")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect()
    }
}
