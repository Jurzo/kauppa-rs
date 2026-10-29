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
use crate::domain::{
    Category, Direction, Product, ProductGroup, Recipe, ShoppingList, User, ordering,
};
use crate::error::CoreError;
use crate::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo, UserRepo};

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

fn row_to_product(row: &SqliteRow) -> CoreResult<Product> {
    let id: String = row.try_get("id")?;
    let owner_id: String = row.try_get("owner_id")?;
    let category_id: Option<String> = row.try_get("category_id")?;
    Ok(Product {
        id: parse_uuid(&id)?,
        owner_id: parse_uuid(&owner_id)?,
        name: row.try_get("name")?,
        category_id: category_id.as_deref().map(parse_uuid).transpose()?,
        position: row.try_get("position")?,
        is_favorite: row.try_get("is_favorite")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// Trim a user-supplied name and reject it if empty. `what` names the entity
/// in the error message.
fn clean_name<'a>(what: &str, name: &'a str) -> CoreResult<&'a str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CoreError::Invalid(format!("{what} name must not be empty")));
    }
    Ok(trimmed)
}

/// Map a unique-constraint violation to `Conflict`; pass other errors through.
fn conflict_on_unique(e: sqlx::Error, message: impl FnOnce() -> String) -> CoreError {
    match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => CoreError::Conflict(message()),
        _ => CoreError::Database(e),
    }
}

fn unknown_category() -> CoreError {
    CoreError::Invalid("unknown category".to_owned())
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
        let name = clean_name("category", name)?;
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
        let name = clean_name("category", name)?;
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
impl ProductRepo for SqlitePool {
    async fn create(&self, owner_id: Uuid, name: &str, category_id: Uuid) -> CoreResult<Product> {
        let name = clean_name("product", name)?;
        let now = Utc::now();

        // One statement: nothing is inserted unless the category is the
        // owner's, and the product is appended at the end of that category.
        let row = sqlx::query(
            r"INSERT INTO products
                  (id, owner_id, name, category_id, position, is_favorite, created_at, updated_at)
              SELECT ?1, ?2, ?3, c.id,
                     (SELECT COALESCE(MAX(p.position), -1) + 1
                        FROM products p
                        WHERE p.owner_id = ?2 AND p.category_id = c.id),
                     1, ?4, ?4
                FROM categories c
                WHERE c.id = ?5 AND c.owner_id = ?2
              RETURNING id, owner_id, name, category_id, position, is_favorite,
                        created_at, updated_at",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(owner_id.to_string())
        .bind(name)
        .bind(now)
        .bind(category_id.to_string())
        .fetch_optional(&self.inner)
        .await
        .map_err(|e| conflict_on_unique(e, || format!("product already exists: {name}")))?;

        row.as_ref()
            .map(row_to_product)
            .transpose()?
            .ok_or_else(unknown_category)
    }

    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ProductGroup>> {
        // The join also checks the category's owner, so a product can never be
        // shown under someone else's category; it would fall to uncategorized.
        let rows = sqlx::query(
            r"SELECT p.id, p.owner_id, p.name, p.category_id, p.position, p.is_favorite,
                     p.created_at, p.updated_at,
                     c.id AS group_id, c.name AS group_name,
                     c.position AS group_position, c.created_at AS group_created_at
                FROM products p
                LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = p.owner_id
                WHERE p.owner_id = ?1
                ORDER BY c.id IS NULL, c.position, c.name, p.position, p.name",
        )
        .bind(owner_id.to_string())
        .fetch_all(&self.inner)
        .await?;

        let mut groups: Vec<ProductGroup> = Vec::new();
        for row in &rows {
            let product = row_to_product(row)?;
            let group_id: Option<String> = row.try_get("group_id")?;
            let group_id = group_id.as_deref().map(parse_uuid).transpose()?;

            match groups.last_mut() {
                Some(group) if group.category.as_ref().map(|c| c.id) == group_id => {
                    group.products.push(product);
                }
                _ => {
                    let category = group_id
                        .map(|id| -> CoreResult<Category> {
                            Ok(Category {
                                id,
                                owner_id,
                                name: row.try_get("group_name")?,
                                position: row.try_get("group_position")?,
                                created_at: row.try_get("group_created_at")?,
                            })
                        })
                        .transpose()?;
                    groups.push(ProductGroup {
                        category,
                        products: vec![product],
                    });
                }
            }
        }
        Ok(groups)
    }

    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<Product> {
        let name = clean_name("product", name)?;

        let row = sqlx::query(
            r"UPDATE products SET name = ?1, updated_at = ?2
                WHERE id = ?3 AND owner_id = ?4
              RETURNING id, owner_id, name, category_id, position, is_favorite,
                        created_at, updated_at",
        )
        .bind(name)
        .bind(Utc::now())
        .bind(id.to_string())
        .bind(owner_id.to_string())
        .fetch_optional(&self.inner)
        .await
        .map_err(|e| conflict_on_unique(e, || format!("product already exists: {name}")))?;

        row.as_ref()
            .map(row_to_product)
            .transpose()?
            .ok_or(CoreError::NotFound)
    }

    async fn recategorize(
        &self,
        owner_id: Uuid,
        id: Uuid,
        category_id: Uuid,
    ) -> CoreResult<Product> {
        let owner = owner_id.to_string();
        let id_str = id.to_string();
        let category = category_id.to_string();
        // IMMEDIATE takes the write lock up front, so the reads below can't be
        // invalidated by another writer before the update.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;

        let row = sqlx::query(
            r"SELECT id, owner_id, name, category_id, position, is_favorite,
                     created_at, updated_at
                FROM products
                WHERE id = ?1 AND owner_id = ?2",
        )
        .bind(&id_str)
        .bind(&owner)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CoreError::NotFound)?;
        let product = row_to_product(&row)?;
        if product.category_id == Some(category_id) {
            // Already there: keep its position rather than moving it to the end.
            return Ok(product);
        }

        let category_owned =
            sqlx::query(r"SELECT 1 FROM categories WHERE id = ?1 AND owner_id = ?2")
                .bind(&category)
                .bind(&owner)
                .fetch_optional(&mut *tx)
                .await?
                .is_some();
        if !category_owned {
            return Err(unknown_category());
        }

        let row = sqlx::query(
            r"UPDATE products
                SET category_id = ?1,
                    position = (SELECT COALESCE(MAX(p.position), -1) + 1
                                  FROM products p
                                  WHERE p.owner_id = ?2 AND p.category_id = ?1),
                    updated_at = ?3
                WHERE id = ?4 AND owner_id = ?2
              RETURNING id, owner_id, name, category_id, position, is_favorite,
                        created_at, updated_at",
        )
        .bind(&category)
        .bind(&owner)
        .bind(Utc::now())
        .bind(&id_str)
        .fetch_one(&mut *tx)
        .await?;
        let product = row_to_product(&row)?;

        tx.commit().await?;
        Ok(product)
    }

    async fn move_within_category(
        &self,
        owner_id: Uuid,
        id: Uuid,
        direction: Direction,
    ) -> CoreResult<()> {
        let owner = owner_id.to_string();
        let id_str = id.to_string();
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;

        let category_id: Option<String> =
            sqlx::query(r"SELECT category_id FROM products WHERE id = ?1 AND owner_id = ?2")
                .bind(&id_str)
                .bind(&owner)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(CoreError::NotFound)?
                .try_get("category_id")?;

        // `IS` so the uncategorized (NULL) group can be ordered too.
        let siblings = sqlx::query(
            r"SELECT id FROM products
                WHERE owner_id = ?1 AND category_id IS ?2
                ORDER BY position ASC, name ASC",
        )
        .bind(&owner)
        .bind(category_id.as_deref())
        .fetch_all(&mut *tx)
        .await?;
        let mut ids = siblings
            .iter()
            .map(|row| row.try_get::<String, _>("id"))
            .collect::<Result<Vec<_>, _>>()?;

        let index = ids
            .iter()
            .position(|p| *p == id_str)
            .ok_or(CoreError::NotFound)?;
        if !ordering::step(&mut ids, index, direction) {
            return Ok(());
        }

        // Rewrite the whole group densely, which also heals any ties or gaps.
        for (index, product_id) in ids.iter().enumerate() {
            let position = i64::try_from(index)
                .map_err(|_| CoreError::Invalid("too many products".to_owned()))?;
            sqlx::query(r"UPDATE products SET position = ?1 WHERE id = ?2 AND owner_id = ?3")
                .bind(position)
                .bind(product_id)
                .bind(&owner)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()> {
        let result = sqlx::query(r"DELETE FROM products WHERE id = ?1 AND owner_id = ?2")
            .bind(id.to_string())
            .bind(owner_id.to_string())
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
