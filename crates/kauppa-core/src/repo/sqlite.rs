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
    Category, Direction, Ingredient, ListItem, NewIngredient, Product, ProductGroup, Recipe,
    RecipeDetail, RecipeInput, ShoppingList, User, ordering,
};
use crate::error::CoreError;
use crate::repo::name_key::name_key;
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
            r"INSERT INTO categories (id, owner_id, name, name_key, position, created_at)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id_str)
        .bind(&owner)
        .bind(name)
        .bind(name_key(name))
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

        let result = sqlx::query(
            r"UPDATE categories SET name = ?1, name_key = ?2 WHERE id = ?3 AND owner_id = ?4",
        )
        .bind(name)
        .bind(name_key(name))
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
        let owner = owner_id.to_string();
        let category = category_id.to_string();
        // IMMEDIATE: the hidden-product check and the insert can't race another
        // writer creating the same name.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;

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

        // A hidden product (created from a recipe ingredient) of this name is
        // promoted rather than duplicated: one product, one row (RR-0001).
        let promoted = sqlx::query(
            r"UPDATE products
                SET is_favorite = 1,
                    category_id = ?1,
                    position = (SELECT COALESCE(MAX(p.position), -1) + 1
                                  FROM products p
                                  WHERE p.owner_id = ?2 AND p.category_id = ?1),
                    updated_at = ?3
                WHERE owner_id = ?2 AND name_key = ?4 AND is_favorite = 0
              RETURNING id, owner_id, name, category_id, position, is_favorite,
                        created_at, updated_at",
        )
        .bind(&category)
        .bind(&owner)
        .bind(now)
        .bind(name_key(name))
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = promoted {
            let product = row_to_product(&row)?;
            tx.commit().await?;
            return Ok(product);
        }

        // Nothing is inserted unless the category is the owner's, and the
        // product is appended at the end of that category.
        let row = sqlx::query(
            r"INSERT INTO products
                  (id, owner_id, name, name_key, category_id, position, is_favorite,
                   created_at, updated_at)
              SELECT ?1, ?2, ?3, ?6, c.id,
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
        .bind(&owner)
        .bind(name)
        .bind(now)
        .bind(&category)
        .bind(name_key(name))
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| conflict_on_unique(e, || format!("product already exists: {name}")))?;

        let product = row
            .as_ref()
            .map(row_to_product)
            .transpose()?
            .ok_or_else(unknown_category)?;
        tx.commit().await?;
        Ok(product)
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
                WHERE p.owner_id = ?1 AND p.is_favorite = 1
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

    async fn find_by_name(&self, owner_id: Uuid, name: &str) -> CoreResult<Option<Product>> {
        let name = clean_name("product", name)?;
        let row = sqlx::query(
            r"SELECT id, owner_id, name, category_id, position, is_favorite,
                     created_at, updated_at
                FROM products
                WHERE owner_id = ?1 AND name_key = ?2",
        )
        .bind(owner_id.to_string())
        .bind(name_key(name))
        .fetch_optional(&self.inner)
        .await?;

        row.as_ref().map(row_to_product).transpose()
    }

    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<Product> {
        let name = clean_name("product", name)?;

        let row = sqlx::query(
            r"UPDATE products SET name = ?1, name_key = ?5, updated_at = ?2
                WHERE id = ?3 AND owner_id = ?4
              RETURNING id, owner_id, name, category_id, position, is_favorite,
                        created_at, updated_at",
        )
        .bind(name)
        .bind(Utc::now())
        .bind(id.to_string())
        .bind(owner_id.to_string())
        .bind(name_key(name))
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
            r"SELECT id, is_favorite FROM products
                WHERE owner_id = ?1 AND category_id IS ?2
                ORDER BY position ASC, name ASC",
        )
        .bind(&owner)
        .bind(category_id.as_deref())
        .fetch_all(&mut *tx)
        .await?;
        let mut ids = Vec::with_capacity(siblings.len());
        let mut favorite = Vec::with_capacity(siblings.len());
        for row in &siblings {
            ids.push(row.try_get::<String, _>("id")?);
            favorite.push(row.try_get::<bool, _>("is_favorite")?);
        }

        // Step among the products the user sees: a favorite swaps places with
        // the next favorite, and hidden products in between keep their slots.
        // A hidden product (not reachable from the favorites views) steps
        // among all products.
        let target = ids
            .iter()
            .position(|p| *p == id_str)
            .ok_or(CoreError::NotFound)?;
        let slots: Vec<usize> = if favorite[target] {
            (0..ids.len()).filter(|&i| favorite[i]).collect()
        } else {
            (0..ids.len()).collect()
        };
        let mut visible: Vec<String> = slots.iter().map(|&i| ids[i].clone()).collect();
        let index = slots
            .iter()
            .position(|&i| i == target)
            .ok_or(CoreError::NotFound)?;
        if !ordering::step(&mut visible, index, direction) {
            return Ok(());
        }
        for (&slot, product_id) in slots.iter().zip(visible) {
            ids[slot] = product_id;
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

fn row_to_list(row: &SqliteRow) -> CoreResult<ShoppingList> {
    let id: String = row.try_get("id")?;
    let owner_id: String = row.try_get("owner_id")?;
    Ok(ShoppingList {
        id: parse_uuid(&id)?,
        owner_id: parse_uuid(&owner_id)?,
        name: row.try_get("name")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// Map a row of [`ITEM_SELECT`] to a list item.
fn row_to_item(row: &SqliteRow, owner_id: Uuid) -> CoreResult<ListItem> {
    let id: String = row.try_get("id")?;
    let list_id: String = row.try_get("list_id")?;
    let product_id: Option<String> = row.try_get("product_id")?;
    let category_id: Option<String> = row.try_get("category_id")?;
    let category = category_id
        .as_deref()
        .map(|id| -> CoreResult<Category> {
            Ok(Category {
                id: parse_uuid(id)?,
                owner_id,
                name: row.try_get("category_name")?,
                position: row.try_get("category_position")?,
                created_at: row.try_get("category_created_at")?,
            })
        })
        .transpose()?;
    Ok(ListItem {
        id: parse_uuid(&id)?,
        list_id: parse_uuid(&list_id)?,
        product_id: product_id.as_deref().map(parse_uuid).transpose()?,
        text: row.try_get("text")?,
        quantity: row.try_get("quantity")?,
        checked: row.try_get("checked")?,
        position: row.try_get("position")?,
        category,
        removed_at: row.try_get("removed_at")?,
    })
}

/// A list's items with their categories' live names and order. Binds: `?1`
/// list id, `?2` owner id, `?3` one item id or NULL for all, `?4` 1 for
/// removed items or 0 for active ones (ignored when `?3` is set). Every join
/// checks the owner (RR-0001 constraint 9).
const ITEM_SELECT: &str = r"
    SELECT i.id, i.list_id, p.id AS product_id, i.text, i.quantity, i.checked, i.position,
           i.removed_at,
           c.id AS category_id, c.name AS category_name,
           c.position AS category_position, c.created_at AS category_created_at
      FROM list_items i
      JOIN shopping_lists l ON l.id = i.list_id AND l.owner_id = ?2
      LEFT JOIN products p ON p.id = i.product_id AND p.owner_id = l.owner_id
      LEFT JOIN categories c ON c.id = i.category_id AND c.owner_id = l.owner_id
     WHERE i.list_id = ?1
       AND (CASE WHEN ?3 IS NULL THEN (i.removed_at IS NOT NULL) = ?4 ELSE i.id = ?3 END)
     ORDER BY i.position ASC, i.id ASC";

impl SqlitePool {
    async fn fetch_items(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        only: Option<Uuid>,
        removed: bool,
    ) -> CoreResult<Vec<ListItem>> {
        let rows = sqlx::query(ITEM_SELECT)
            .bind(list_id.to_string())
            .bind(owner_id.to_string())
            .bind(only.map(|id| id.to_string()))
            .bind(removed)
            .fetch_all(&self.inner)
            .await?;
        rows.iter().map(|row| row_to_item(row, owner_id)).collect()
    }

    async fn fetch_item(&self, owner_id: Uuid, list_id: Uuid, id: Uuid) -> CoreResult<ListItem> {
        self.fetch_items(owner_id, list_id, Some(id), false)
            .await?
            .into_iter()
            .next()
            .ok_or(CoreError::NotFound)
    }
}

/// Fail with `NotFound` unless `list_id` is `owner`'s, and mark it updated,
/// so a list being worked on rises to the top of the index. Runs inside the
/// caller's transaction so the item write and `updated_at` commit together.
async fn touch_owned_list(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    owner: &str,
    list_id: &str,
) -> CoreResult<()> {
    let result =
        sqlx::query(r"UPDATE shopping_lists SET updated_at = ?1 WHERE id = ?2 AND owner_id = ?3")
            .bind(Utc::now())
            .bind(list_id)
            .bind(owner)
            .execute(&mut **tx)
            .await?;
    if result.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
}

#[async_trait]
impl ShoppingListRepo for SqlitePool {
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ShoppingList>> {
        // `id` as tiebreak keeps the order stable for equal timestamps.
        let rows = sqlx::query(
            r"SELECT id, owner_id, name, created_at, updated_at
                FROM shopping_lists
                WHERE owner_id = ?1
                ORDER BY updated_at DESC, id ASC",
        )
        .bind(owner_id.to_string())
        .fetch_all(&self.inner)
        .await?;

        rows.iter().map(row_to_list).collect()
    }

    async fn create(&self, owner_id: Uuid, name: &str) -> CoreResult<ShoppingList> {
        let name = clean_name("list", name)?;
        let row = sqlx::query(
            r"INSERT INTO shopping_lists (id, owner_id, name, created_at, updated_at)
              VALUES (?1, ?2, ?3, ?4, ?4)
              RETURNING id, owner_id, name, created_at, updated_at",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(owner_id.to_string())
        .bind(name)
        .bind(Utc::now())
        .fetch_one(&self.inner)
        .await?;

        row_to_list(&row)
    }

    async fn get(&self, owner_id: Uuid, id: Uuid) -> CoreResult<ShoppingList> {
        let row = sqlx::query(
            r"SELECT id, owner_id, name, created_at, updated_at
                FROM shopping_lists
                WHERE id = ?1 AND owner_id = ?2",
        )
        .bind(id.to_string())
        .bind(owner_id.to_string())
        .fetch_optional(&self.inner)
        .await?;

        row.as_ref()
            .map(row_to_list)
            .transpose()?
            .ok_or(CoreError::NotFound)
    }

    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<ShoppingList> {
        let name = clean_name("list", name)?;
        let row = sqlx::query(
            r"UPDATE shopping_lists SET name = ?1, updated_at = ?2
                WHERE id = ?3 AND owner_id = ?4
              RETURNING id, owner_id, name, created_at, updated_at",
        )
        .bind(name)
        .bind(Utc::now())
        .bind(id.to_string())
        .bind(owner_id.to_string())
        .fetch_optional(&self.inner)
        .await?;

        row.as_ref()
            .map(row_to_list)
            .transpose()?
            .ok_or(CoreError::NotFound)
    }

    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()> {
        let result = sqlx::query(r"DELETE FROM shopping_lists WHERE id = ?1 AND owner_id = ?2")
            .bind(id.to_string())
            .bind(owner_id.to_string())
            .execute(&self.inner)
            .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }

    async fn items(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        removed: bool,
    ) -> CoreResult<Vec<ListItem>> {
        // An unknown or foreign list is NotFound, not an empty list.
        ShoppingListRepo::get(self, owner_id, list_id).await?;
        self.fetch_items(owner_id, list_id, None, removed).await
    }

    async fn add_item(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        text: &str,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem> {
        let text = clean_name("item", text)?;
        let list = list_id.to_string();
        let id = Uuid::new_v4();
        let mut tx = self.inner.begin().await?;
        touch_owned_list(&mut tx, &owner_id.to_string(), &list).await?;

        // Appended at the end; removed items keep their positions, so a
        // restored item returns to its place.
        sqlx::query(
            r"INSERT INTO list_items (id, list_id, text, quantity, position)
              VALUES (?1, ?2, ?3, ?4,
                      (SELECT COALESCE(MAX(position), -1) + 1
                         FROM list_items WHERE list_id = ?2))",
        )
        .bind(id.to_string())
        .bind(&list)
        .bind(text)
        .bind(clean_quantity(quantity))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.fetch_item(owner_id, list_id, id).await
    }

    async fn add_product(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        product_id: Uuid,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem> {
        let owner = owner_id.to_string();
        let list = list_id.to_string();
        let id = Uuid::new_v4();
        let mut tx = self.inner.begin().await?;
        touch_owned_list(&mut tx, &owner, &list).await?;

        // One statement: the item copies the product's name and category, and
        // nothing is inserted unless the product is the owner's.
        let inserted = sqlx::query(
            r"INSERT INTO list_items
                  (id, list_id, product_id, text, category_id, quantity, position)
              SELECT ?1, ?2, p.id, p.name, p.category_id, ?3,
                     (SELECT COALESCE(MAX(position), -1) + 1
                        FROM list_items WHERE list_id = ?2)
                FROM products p
                WHERE p.id = ?4 AND p.owner_id = ?5",
        )
        .bind(id.to_string())
        .bind(&list)
        .bind(clean_quantity(quantity))
        .bind(product_id.to_string())
        .bind(&owner)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 0 {
            return Err(CoreError::Invalid("unknown product".to_owned()));
        }
        tx.commit().await?;

        self.fetch_item(owner_id, list_id, id).await
    }

    async fn assign_category(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        item_id: Uuid,
        category_id: Uuid,
    ) -> CoreResult<ListItem> {
        let owner = owner_id.to_string();
        let list = list_id.to_string();
        let mut tx = self.inner.begin().await?;
        touch_owned_list(&mut tx, &owner, &list).await?;

        let item_exists = sqlx::query(r"SELECT 1 FROM list_items WHERE id = ?1 AND list_id = ?2")
            .bind(item_id.to_string())
            .bind(&list)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
        if !item_exists {
            return Err(CoreError::NotFound);
        }
        // The category must be the owner's (RR-0001 constraint 9).
        let updated = sqlx::query(
            r"UPDATE list_items SET category_id = ?1
                WHERE id = ?3 AND list_id = ?4
                  AND EXISTS (SELECT 1 FROM categories WHERE id = ?1 AND owner_id = ?2)",
        )
        .bind(category_id.to_string())
        .bind(&owner)
        .bind(item_id.to_string())
        .bind(&list)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(unknown_category());
        }
        tx.commit().await?;

        self.fetch_item(owner_id, list_id, item_id).await
    }

    async fn remove_item(&self, owner_id: Uuid, list_id: Uuid, item_id: Uuid) -> CoreResult<()> {
        let list = list_id.to_string();
        let mut tx = self.inner.begin().await?;
        touch_owned_list(&mut tx, &owner_id.to_string(), &list).await?;

        let result = sqlx::query(
            r"UPDATE list_items SET removed_at = ?1
                WHERE id = ?2 AND list_id = ?3 AND removed_at IS NULL",
        )
        .bind(Utc::now())
        .bind(item_id.to_string())
        .bind(&list)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }

        tx.commit().await?;
        Ok(())
    }

    async fn restore_item(&self, owner_id: Uuid, list_id: Uuid, item_id: Uuid) -> CoreResult<()> {
        let list = list_id.to_string();
        let mut tx = self.inner.begin().await?;
        touch_owned_list(&mut tx, &owner_id.to_string(), &list).await?;

        let result = sqlx::query(
            r"UPDATE list_items SET removed_at = NULL
                WHERE id = ?1 AND list_id = ?2 AND removed_at IS NOT NULL",
        )
        .bind(item_id.to_string())
        .bind(&list)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }

        tx.commit().await?;
        Ok(())
    }
}

fn row_to_recipe(row: &SqliteRow) -> CoreResult<Recipe> {
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
}

/// Map a row of [`INGREDIENT_SELECT`] to an ingredient.
fn row_to_ingredient(row: &SqliteRow, owner_id: Uuid) -> CoreResult<Ingredient> {
    let id: String = row.try_get("id")?;
    let recipe_id: String = row.try_get("recipe_id")?;
    let product_id: Option<String> = row.try_get("product_id")?;
    let category_id: Option<String> = row.try_get("category_id")?;
    let category = category_id
        .as_deref()
        .map(|id| -> CoreResult<Category> {
            Ok(Category {
                id: parse_uuid(id)?,
                owner_id,
                name: row.try_get("category_name")?,
                position: row.try_get("category_position")?,
                created_at: row.try_get("category_created_at")?,
            })
        })
        .transpose()?;
    Ok(Ingredient {
        id: parse_uuid(&id)?,
        recipe_id: parse_uuid(&recipe_id)?,
        product_id: product_id.as_deref().map(parse_uuid).transpose()?,
        text: row.try_get("text")?,
        quantity: row.try_get("quantity")?,
        position: row.try_get("position")?,
        category,
    })
}

/// A recipe's ingredients with their products' live names and categories.
/// Binds: `?1` recipe id, `?2` owner id, `?3` one ingredient id or NULL for
/// all. Every join checks the owner, so another user's product or category
/// can never show up here (RR-0001 constraint 9).
const INGREDIENT_SELECT: &str = r"
    SELECT i.id, i.recipe_id, p.id AS product_id, COALESCE(p.name, i.text) AS text,
           i.quantity, i.position,
           c.id AS category_id, c.name AS category_name,
           c.position AS category_position, c.created_at AS category_created_at
      FROM ingredients i
      JOIN recipes r ON r.id = i.recipe_id AND r.owner_id = ?2
      LEFT JOIN products p ON p.id = i.product_id AND p.owner_id = r.owner_id
      LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = r.owner_id
     WHERE i.recipe_id = ?1 AND (?3 IS NULL OR i.id = ?3)
     ORDER BY i.position ASC, i.id ASC";

/// Trim a recipe's fields and check them: a non-empty title, servings ≥ 1.
fn clean_recipe(input: &RecipeInput) -> CoreResult<RecipeInput> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(CoreError::Invalid(
            "recipe title must not be empty".to_owned(),
        ));
    }
    if input.servings.is_some_and(|n| n < 1) {
        return Err(CoreError::Invalid("servings must be at least 1".to_owned()));
    }
    Ok(RecipeInput {
        title: title.to_owned(),
        instructions: input.instructions.trim().to_owned(),
        servings: input.servings,
    })
}

/// Trim a quantity; blank means "no quantity".
fn clean_quantity(quantity: Option<&str>) -> Option<&str> {
    quantity.map(str::trim).filter(|q| !q.is_empty())
}

impl SqlitePool {
    async fn fetch_ingredients(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        only: Option<Uuid>,
    ) -> CoreResult<Vec<Ingredient>> {
        let rows = sqlx::query(INGREDIENT_SELECT)
            .bind(recipe_id.to_string())
            .bind(owner_id.to_string())
            .bind(only.map(|id| id.to_string()))
            .fetch_all(&self.inner)
            .await?;
        rows.iter()
            .map(|row| row_to_ingredient(row, owner_id))
            .collect()
    }
}

/// Fail with `NotFound` unless `recipe_id` is `owner`'s, and mark it updated.
/// Runs inside the caller's transaction so ingredient writes and the recipe's
/// `updated_at` commit together.
async fn touch_owned_recipe(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    owner: &str,
    recipe_id: &str,
) -> CoreResult<()> {
    let result = sqlx::query(r"UPDATE recipes SET updated_at = ?1 WHERE id = ?2 AND owner_id = ?3")
        .bind(Utc::now())
        .bind(recipe_id)
        .bind(owner)
        .execute(&mut **tx)
        .await?;
    if result.rows_affected() == 0 {
        return Err(CoreError::NotFound);
    }
    Ok(())
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

        rows.iter().map(row_to_recipe).collect()
    }

    async fn create(&self, owner_id: Uuid, input: &RecipeInput) -> CoreResult<Recipe> {
        let input = clean_recipe(input)?;
        let row = sqlx::query(
            r"INSERT INTO recipes
                  (id, owner_id, title, instructions, servings, created_at, updated_at)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
              RETURNING id, owner_id, title, instructions, servings, created_at, updated_at",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(owner_id.to_string())
        .bind(&input.title)
        .bind(&input.instructions)
        .bind(input.servings)
        .bind(Utc::now())
        .fetch_one(&self.inner)
        .await?;

        row_to_recipe(&row)
    }

    async fn get(&self, owner_id: Uuid, id: Uuid) -> CoreResult<RecipeDetail> {
        let row = sqlx::query(
            r"SELECT id, owner_id, title, instructions, servings, created_at, updated_at
                FROM recipes
                WHERE id = ?1 AND owner_id = ?2",
        )
        .bind(id.to_string())
        .bind(owner_id.to_string())
        .fetch_optional(&self.inner)
        .await?
        .ok_or(CoreError::NotFound)?;

        Ok(RecipeDetail {
            recipe: row_to_recipe(&row)?,
            ingredients: self.fetch_ingredients(owner_id, id, None).await?,
        })
    }

    async fn update(&self, owner_id: Uuid, id: Uuid, input: &RecipeInput) -> CoreResult<Recipe> {
        let input = clean_recipe(input)?;
        let row = sqlx::query(
            r"UPDATE recipes
                SET title = ?1, instructions = ?2, servings = ?3, updated_at = ?4
                WHERE id = ?5 AND owner_id = ?6
              RETURNING id, owner_id, title, instructions, servings, created_at, updated_at",
        )
        .bind(&input.title)
        .bind(&input.instructions)
        .bind(input.servings)
        .bind(Utc::now())
        .bind(id.to_string())
        .bind(owner_id.to_string())
        .fetch_optional(&self.inner)
        .await?;

        row.as_ref()
            .map(row_to_recipe)
            .transpose()?
            .ok_or(CoreError::NotFound)
    }

    async fn add_ingredient(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient: &NewIngredient,
    ) -> CoreResult<Ingredient> {
        let name = clean_name("product", &ingredient.product_name)?;
        let owner = owner_id.to_string();
        let recipe = recipe_id.to_string();
        let now = Utc::now();
        // IMMEDIATE: the product lookup and its creation can't race another
        // writer adding the same name.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        touch_owned_recipe(&mut tx, &owner, &recipe).await?;

        // Reuse the owner's product of this name (MB-0003 P3, matched through
        // name_key per RR-0003 constraint 3); its category is kept as is.
        let existing =
            sqlx::query(r"SELECT id, name FROM products WHERE owner_id = ?1 AND name_key = ?2")
                .bind(&owner)
                .bind(name_key(name))
                .fetch_optional(&mut *tx)
                .await?;
        let (product_id, product_name): (String, String) = if let Some(row) = existing {
            (row.try_get("id")?, row.try_get("name")?)
        } else {
            let category = ingredient.category_id.ok_or_else(|| {
                CoreError::Invalid(format!("choose a category for the new product {name}"))
            })?;
            // A new product, hidden from favorites, appended at the end of an
            // owned category; nothing is inserted for a foreign one.
            let row = sqlx::query(
                r"INSERT INTO products
                      (id, owner_id, name, name_key, category_id, position, is_favorite,
                       created_at, updated_at)
                  SELECT ?1, ?2, ?3, ?4, c.id,
                         (SELECT COALESCE(MAX(p.position), -1) + 1
                            FROM products p
                            WHERE p.owner_id = ?2 AND p.category_id = c.id),
                         0, ?5, ?5
                    FROM categories c
                    WHERE c.id = ?6 AND c.owner_id = ?2
                  RETURNING id",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&owner)
            .bind(name)
            .bind(name_key(name))
            .bind(now)
            .bind(category.to_string())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| conflict_on_unique(e, || format!("product already exists: {name}")))?
            .ok_or_else(unknown_category)?;
            (row.try_get("id")?, name.to_owned())
        };

        let id = Uuid::new_v4();
        sqlx::query(
            r"INSERT INTO ingredients (id, recipe_id, product_id, text, quantity, position)
              VALUES (?1, ?2, ?3, ?4, ?5,
                      (SELECT COALESCE(MAX(position), -1) + 1
                         FROM ingredients WHERE recipe_id = ?2))",
        )
        .bind(id.to_string())
        .bind(&recipe)
        .bind(&product_id)
        .bind(&product_name)
        .bind(clean_quantity(ingredient.quantity.as_deref()))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.fetch_ingredients(owner_id, recipe_id, Some(id))
            .await?
            .into_iter()
            .next()
            .ok_or(CoreError::NotFound)
    }

    async fn set_ingredient_quantity(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient_id: Uuid,
        quantity: Option<&str>,
    ) -> CoreResult<()> {
        let recipe = recipe_id.to_string();
        let mut tx = self.inner.begin().await?;
        touch_owned_recipe(&mut tx, &owner_id.to_string(), &recipe).await?;

        let result =
            sqlx::query(r"UPDATE ingredients SET quantity = ?1 WHERE id = ?2 AND recipe_id = ?3")
                .bind(clean_quantity(quantity))
                .bind(ingredient_id.to_string())
                .bind(&recipe)
                .execute(&mut *tx)
                .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }

        tx.commit().await?;
        Ok(())
    }

    async fn move_ingredient(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient_id: Uuid,
        direction: Direction,
    ) -> CoreResult<()> {
        let recipe = recipe_id.to_string();
        let id_str = ingredient_id.to_string();
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        touch_owned_recipe(&mut tx, &owner_id.to_string(), &recipe).await?;

        let rows = sqlx::query(
            r"SELECT id FROM ingredients WHERE recipe_id = ?1 ORDER BY position ASC, id ASC",
        )
        .bind(&recipe)
        .fetch_all(&mut *tx)
        .await?;
        let mut ids = rows
            .iter()
            .map(|row| row.try_get::<String, _>("id"))
            .collect::<Result<Vec<_>, _>>()?;

        let index = ids
            .iter()
            .position(|i| *i == id_str)
            .ok_or(CoreError::NotFound)?;
        if !ordering::step(&mut ids, index, direction) {
            return Ok(());
        }

        // Rewrite the whole list densely, which also heals gaps left by removals.
        for (index, id) in ids.iter().enumerate() {
            let position = i64::try_from(index)
                .map_err(|_| CoreError::Invalid("too many ingredients".to_owned()))?;
            sqlx::query(r"UPDATE ingredients SET position = ?1 WHERE id = ?2 AND recipe_id = ?3")
                .bind(position)
                .bind(id)
                .bind(&recipe)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn remove_ingredient(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient_id: Uuid,
    ) -> CoreResult<()> {
        let recipe = recipe_id.to_string();
        let mut tx = self.inner.begin().await?;
        touch_owned_recipe(&mut tx, &owner_id.to_string(), &recipe).await?;

        let result = sqlx::query(r"DELETE FROM ingredients WHERE id = ?1 AND recipe_id = ?2")
            .bind(ingredient_id.to_string())
            .bind(&recipe)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }

        tx.commit().await?;
        Ok(())
    }
}
