//! SQLite-backed persistence.
//!
//! Uses runtime-checked `sqlx::query` so the scaffold builds without
//! `DATABASE_URL`. Once the schema is stable, switch to the `query!` macros +
//! `cargo sqlx prepare` offline cache for compile-time checking — tracked in
//! `.agentic/mentorship/persistence.md`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteRow};
use std::str::FromStr;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{Category, Recipe, ShoppingList, User};
use crate::error::CoreError;
use crate::repo::{CategoryRepo, RecipeRepo, ShoppingListRepo, UserRepo};

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

fn row_to_category(row: &SqliteRow) -> CoreResult<Category> {
    let id: String = row.try_get("id")?;
    let owner_id: String = row.try_get("owner_id")?;
    Ok(Category {
        id: parse_uuid(&id)?,
        owner_id: parse_uuid(&owner_id)?,
        name: row.try_get("name")?,
        position: row.try_get("position")?,
        created_at: row.try_get("created_at")?,
    })
}

/// Trim a user-supplied name and reject it if empty.
fn clean_name(name: &str) -> CoreResult<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CoreError::Invalid(
            "category name must not be empty".to_owned(),
        ));
    }
    Ok(trimmed)
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
impl CategoryRepo for SqlitePool {
    async fn create(&self, owner_id: Uuid, name: &str) -> CoreResult<Category> {
        let name = clean_name(name)?;
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let owner = owner_id.to_string();
        let now = Utc::now();

        // Append at the end of this owner's ordering.
        let position: i64 = sqlx::query(
            r"SELECT COALESCE(MAX(position), -1) + 1 AS next
                FROM categories
                WHERE owner_id = ?1",
        )
        .bind(&owner)
        .fetch_one(&self.inner)
        .await?
        .try_get("next")?;

        sqlx::query(
            r"INSERT INTO categories (id, owner_id, name, position, created_at)
              VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(&id_str)
        .bind(&owner)
        .bind(name)
        .bind(position)
        .bind(now)
        .execute(&self.inner)
        .await
        .map_err(|e| match &e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                CoreError::Conflict(format!("category already exists: {name}"))
            }
            _ => CoreError::Database(e),
        })?;

        Ok(Category {
            id,
            owner_id,
            name: name.to_owned(),
            position,
            created_at: now,
        })
    }

    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Category>> {
        let owner = owner_id.to_string();
        let rows = sqlx::query(
            r"SELECT id, owner_id, name, position, created_at
                FROM categories
                WHERE owner_id = ?1
                ORDER BY position ASC, name ASC",
        )
        .bind(&owner)
        .fetch_all(&self.inner)
        .await?;

        rows.iter().map(row_to_category).collect()
    }

    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<Category> {
        let name = clean_name(name)?;
        let owner = owner_id.to_string();
        let id_str = id.to_string();

        let result =
            sqlx::query(r"UPDATE categories SET name = ?1 WHERE id = ?2 AND owner_id = ?3")
                .bind(name)
                .bind(&id_str)
                .bind(&owner)
                .execute(&self.inner)
                .await
                .map_err(|e| match &e {
                    sqlx::Error::Database(db) if db.is_unique_violation() => {
                        CoreError::Conflict(format!("category already exists: {name}"))
                    }
                    _ => CoreError::Database(e),
                })?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }

        let row = sqlx::query(
            r"SELECT id, owner_id, name, position, created_at
                FROM categories
                WHERE id = ?1 AND owner_id = ?2",
        )
        .bind(&id_str)
        .bind(&owner)
        .fetch_one(&self.inner)
        .await?;

        row_to_category(&row)
    }

    async fn reorder(&self, owner_id: Uuid, ordered_ids: &[Uuid]) -> CoreResult<()> {
        let owner = owner_id.to_string();
        let mut tx = self.inner.begin().await?;

        for (index, id) in ordered_ids.iter().enumerate() {
            let position = i64::try_from(index)
                .map_err(|_| CoreError::Invalid("too many categories".to_owned()))?;
            sqlx::query(r"UPDATE categories SET position = ?1 WHERE id = ?2 AND owner_id = ?3")
                .bind(position)
                .bind(id.to_string())
                .bind(&owner)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()> {
        let owner = owner_id.to_string();
        let result = sqlx::query(r"DELETE FROM categories WHERE id = ?1 AND owner_id = ?2")
            .bind(id.to_string())
            .bind(&owner)
            .execute(&self.inner)
            .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
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

#[cfg(test)]
mod category {
    //! MB-0001 acceptance tests for the category repository.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// A fresh, isolated in-memory database with migrations applied.
    async fn test_pool() -> SqlitePool {
        let url = format!(
            "sqlite:file:cat-{}?mode=memory&cache=shared",
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

    /// P1: a repo query for one owner never returns another owner's categories.
    #[tokio::test]
    async fn owner_isolation() {
        let pool = test_pool().await;
        let a = make_user(&pool, "a@example.com").await;
        let b = make_user(&pool, "b@example.com").await;

        CategoryRepo::create(&pool, a, "Hevi").await.unwrap();
        CategoryRepo::create(&pool, b, "Pakasteet").await.unwrap();

        let a_cats = CategoryRepo::list_for_owner(&pool, a).await.unwrap();
        assert_eq!(a_cats.len(), 1);
        assert_eq!(a_cats[0].name, "Hevi");
        assert!(a_cats.iter().all(|c| c.owner_id == a));
    }

    /// P2: `list_for_owner` is ordered by position ascending.
    #[tokio::test]
    async fn list_ordered() {
        let pool = test_pool().await;
        let u = make_user(&pool, "u@example.com").await;
        for name in ["First", "Second", "Third"] {
            CategoryRepo::create(&pool, u, name).await.unwrap();
        }

        let cats = CategoryRepo::list_for_owner(&pool, u).await.unwrap();
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["First", "Second", "Third"]);
        assert!(cats.windows(2).all(|w| w[0].position < w[1].position));
    }

    /// P3: reorder persists the new position values.
    #[tokio::test]
    async fn reorder_persists() {
        let pool = test_pool().await;
        let u = make_user(&pool, "u@example.com").await;
        let a = CategoryRepo::create(&pool, u, "A").await.unwrap();
        let b = CategoryRepo::create(&pool, u, "B").await.unwrap();
        let c = CategoryRepo::create(&pool, u, "C").await.unwrap();

        CategoryRepo::reorder(&pool, u, &[c.id, a.id, b.id])
            .await
            .unwrap();

        let cats = CategoryRepo::list_for_owner(&pool, u).await.unwrap();
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["C", "A", "B"]);
    }

    /// P4: empty and owner-duplicate names are rejected; the same name is fine
    /// for a different owner.
    #[tokio::test]
    async fn unique_nonempty_name() {
        let pool = test_pool().await;
        let u = make_user(&pool, "u@example.com").await;

        assert!(matches!(
            CategoryRepo::create(&pool, u, "   ").await,
            Err(CoreError::Invalid(_))
        ));

        CategoryRepo::create(&pool, u, "Hevi").await.unwrap();
        assert!(matches!(
            CategoryRepo::create(&pool, u, "Hevi").await,
            Err(CoreError::Conflict(_))
        ));

        let other = make_user(&pool, "other@example.com").await;
        CategoryRepo::create(&pool, other, "Hevi").await.unwrap();
    }

    /// P5: a category can be renamed and deleted (owner-scoped); after deletion
    /// it is absent from `list_for_owner`.
    #[tokio::test]
    async fn rename_and_delete() {
        let pool = test_pool().await;
        let u = make_user(&pool, "u@example.com").await;
        let other = make_user(&pool, "other@example.com").await;
        let cat = CategoryRepo::create(&pool, u, "Old").await.unwrap();

        let renamed = CategoryRepo::rename(&pool, u, cat.id, "New").await.unwrap();
        assert_eq!(renamed.name, "New");

        // Cross-owner rename / delete must not touch another user's row.
        assert!(matches!(
            CategoryRepo::rename(&pool, other, cat.id, "Hax").await,
            Err(CoreError::NotFound)
        ));
        assert!(matches!(
            CategoryRepo::delete(&pool, other, cat.id).await,
            Err(CoreError::NotFound)
        ));

        CategoryRepo::delete(&pool, u, cat.id).await.unwrap();
        assert!(
            CategoryRepo::list_for_owner(&pool, u)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            CategoryRepo::delete(&pool, u, cat.id).await,
            Err(CoreError::NotFound)
        ));
    }
}
