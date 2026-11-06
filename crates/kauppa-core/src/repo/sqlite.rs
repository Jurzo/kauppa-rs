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
    Category, Direction, Ingredient, ListAccess, ListGroup, ListItem, ListShare, NewIngredient,
    Product, ProductGroup, Recipe, RecipeDetail, RecipeInput, ShareRole, SharedList, ShoppingList,
    User, ordering, quantity,
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

/// The central list access predicate (RR-0002 constraint 1, `security.md` →
/// Authorization): the lists `?1` (the acting user) owns, or holds a
/// `list_shares` grant on, each with the user's access — `owner`, or the
/// grant's role. `?2` narrows it to one list id; NULL means every list.
///
/// This is the only list query that decides access. Every
/// `ShoppingListRepo` method reaches list and item rows through it
/// (`accessible_lists` → `authorize` → `ListGrant`); `repo::share`'s tests
/// fail if a method or query goes around it.
const ACCESS_SELECT: &str = r"
    SELECT l.id, l.owner_id, l.name, l.created_at, l.updated_at,
           u.display_name AS owner_name,
           CASE WHEN l.owner_id = ?1 THEN 'owner' ELSE s.role END AS access
      FROM shopping_lists l
      JOIN users u ON u.id = l.owner_id
      LEFT JOIN list_shares s ON s.list_id = l.id AND s.grantee_id = ?1
     WHERE (l.owner_id = ?1 OR s.grantee_id IS NOT NULL)
       AND (?2 IS NULL OR l.id = ?2)
     ORDER BY l.updated_at DESC, l.id ASC";

/// One row of [`ACCESS_SELECT`].
struct Accessible {
    list: ShoppingList,
    owner_name: String,
    access: ListAccess,
}

fn parse_access(raw: &str) -> CoreResult<ListAccess> {
    if raw == "owner" {
        Ok(ListAccess::Owner)
    } else {
        ShareRole::parse(raw).map(ListAccess::from)
    }
}

/// Run the access predicate for `user`: every list they can reach, or only
/// `only`.
async fn accessible_lists<'e, E>(
    exec: E,
    user: Uuid,
    only: Option<Uuid>,
) -> CoreResult<Vec<Accessible>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query(ACCESS_SELECT)
        .bind(user.to_string())
        .bind(only.map(|id| id.to_string()))
        .fetch_all(exec)
        .await?;
    rows.iter()
        .map(|row| {
            let access: String = row.try_get("access")?;
            Ok(Accessible {
                list: row_to_list(row)?,
                owner_name: row.try_get("owner_name")?,
                access: parse_access(&access)?,
            })
        })
        .collect()
}

/// Proof that the access predicate let a user act on `list` with `access`.
/// Only [`authorize`] makes one, and every query on a list's rows takes one,
/// so no list or item query can run before the predicate has.
#[derive(Debug)]
struct ListGrant {
    list: ShoppingList,
    access: ListAccess,
}

impl ListGrant {
    fn list_id(&self) -> String {
        self.list.id.to_string()
    }

    /// The list owner: their catalog is the one the list's items join
    /// against, whoever is looking (RR-0001 constraint 9).
    fn owner_id(&self) -> String {
        self.list.owner_id.to_string()
    }
}

/// Decide whether `user` may act on `list_id` with at least `required`
/// access. `NotFound` when they have no access at all (the list's existence
/// stays hidden), `Forbidden` when their role is too weak.
async fn authorize<'e, E>(
    exec: E,
    user: Uuid,
    list_id: Uuid,
    required: ListAccess,
) -> CoreResult<ListGrant>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let found = accessible_lists(exec, user, Some(list_id))
        .await?
        .into_iter()
        .next()
        .ok_or(CoreError::NotFound)?;
    if found.access < required {
        return Err(CoreError::Forbidden);
    }
    Ok(ListGrant {
        list: found.list,
        access: found.access,
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
/// list id, `?2` the list owner's id (from a [`ListGrant`]), `?3` one item id
/// or NULL for all, `?4` 1 for removed items or 0 for active ones (ignored
/// when `?3` is set). Every join checks the owner (RR-0001 constraint 9).
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

/// A list's active items for the store view, with the columns of
/// [`ITEM_SELECT`] plus `product_position`: the product's position, but only
/// while the product is still in the item's category (otherwise the position
/// belongs to another category's order). Binds: `?1` list id, `?2` the list
/// owner's id (from a [`ListGrant`]).
const STORE_VIEW_SELECT: &str = r"
    SELECT i.id, i.list_id, p.id AS product_id, i.text, i.quantity, i.checked, i.position,
           i.removed_at,
           c.id AS category_id, c.name AS category_name,
           c.position AS category_position, c.created_at AS category_created_at,
           CASE WHEN p.category_id = c.id THEN p.position END AS product_position
      FROM list_items i
      JOIN shopping_lists l ON l.id = i.list_id AND l.owner_id = ?2
      LEFT JOIN products p ON p.id = i.product_id AND p.owner_id = l.owner_id
      LEFT JOIN categories c ON c.id = i.category_id AND c.owner_id = l.owner_id
     WHERE i.list_id = ?1 AND i.removed_at IS NULL";

/// A list's grants with the grantees' names. Binds: `?1` list id (from a
/// [`ListGrant`]), `?2` one share id or NULL for all.
const SHARE_SELECT: &str = r"
    SELECT s.id, s.list_id, s.grantee_id, u.display_name AS grantee_name,
           u.email AS grantee_email, s.role, s.created_at
      FROM list_shares s
      JOIN users u ON u.id = s.grantee_id
     WHERE s.list_id = ?1 AND (?2 IS NULL OR s.id = ?2)
     ORDER BY s.created_at ASC, s.id ASC";

fn row_to_share(row: &SqliteRow) -> CoreResult<ListShare> {
    let id: String = row.try_get("id")?;
    let list_id: String = row.try_get("list_id")?;
    let grantee_id: String = row.try_get("grantee_id")?;
    let role: String = row.try_get("role")?;
    Ok(ListShare {
        id: parse_uuid(&id)?,
        list_id: parse_uuid(&list_id)?,
        grantee_id: parse_uuid(&grantee_id)?,
        grantee_name: row.try_get("grantee_name")?,
        grantee_email: row.try_get("grantee_email")?,
        role: ShareRole::parse(&role)?,
        created_at: row.try_get("created_at")?,
    })
}

/// Group store-view rows by category and order them (see
/// `ShoppingListRepo::store_view`).
fn group_for_store(mut rows: Vec<(ListItem, Option<i64>)>) -> Vec<ListGroup> {
    // Category order: the owner's position, then name; uncategorized last.
    let group_key = |item: &ListItem| {
        item.category
            .as_ref()
            .map(|c| (c.position, c.name.to_lowercase(), c.id))
    };
    rows.sort_by(|(a, a_pos), (b, b_pos)| {
        let (ga, gb) = (group_key(a), group_key(b));
        ga.is_none()
            .cmp(&gb.is_none())
            .then_with(|| ga.cmp(&gb))
            // Within a group: product position first, then by name.
            .then_with(|| a_pos.is_none().cmp(&b_pos.is_none()))
            .then_with(|| a_pos.cmp(b_pos))
            .then_with(|| a.text.to_lowercase().cmp(&b.text.to_lowercase()))
            .then_with(|| a.position.cmp(&b.position))
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut groups: Vec<ListGroup> = Vec::new();
    for (item, _) in rows {
        match groups.last_mut() {
            Some(group)
                if group.category.as_ref().map(|c| c.id)
                    == item.category.as_ref().map(|c| c.id) =>
            {
                group.items.push(item);
            }
            _ => groups.push(ListGroup {
                category: item.category.clone(),
                items: vec![item],
            }),
        }
    }
    groups
}

/// A list's items (see [`ITEM_SELECT`]), once the predicate has granted
/// access.
async fn fetch_items<'e, E>(
    exec: E,
    grant: &ListGrant,
    only: Option<Uuid>,
    removed: bool,
) -> CoreResult<Vec<ListItem>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query(ITEM_SELECT)
        .bind(grant.list_id())
        .bind(grant.owner_id())
        .bind(only.map(|id| id.to_string()))
        .bind(removed)
        .fetch_all(exec)
        .await?;
    rows.iter()
        .map(|row| row_to_item(row, grant.list.owner_id))
        .collect()
}

async fn fetch_item<'e, E>(exec: E, grant: &ListGrant, id: Uuid) -> CoreResult<ListItem>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    fetch_items(exec, grant, Some(id), false)
        .await?
        .into_iter()
        .next()
        .ok_or(CoreError::NotFound)
}

/// Mark a granted list updated, so a list being worked on rises to the top
/// of the index. Runs inside the caller's transaction so the item write and
/// `updated_at` commit together.
async fn touch_list(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    grant: &ListGrant,
) -> CoreResult<()> {
    sqlx::query(r"UPDATE shopping_lists SET updated_at = ?1 WHERE id = ?2")
        .bind(Utc::now())
        .bind(grant.list_id())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

type Tx = sqlx::Transaction<'static, sqlx::Sqlite>;

impl SqlitePool {
    /// Open a read transaction and authorize `user` on `list_id` inside it,
    /// so the check and the reads that follow see one state.
    async fn begin_read(
        &self,
        user: Uuid,
        list_id: Uuid,
        required: ListAccess,
    ) -> CoreResult<(Tx, ListGrant)> {
        let mut tx = self.inner.begin().await?;
        let grant = authorize(&mut *tx, user, list_id, required).await?;
        Ok((tx, grant))
    }

    /// Open a write transaction and authorize `user` on `list_id` inside it.
    /// `BEGIN IMMEDIATE` takes SQLite's write lock first, so a revoke can't
    /// commit between the check and the write (RR-0002 constraint 4), and a
    /// failure leaves nothing half-written.
    async fn begin_write(
        &self,
        user: Uuid,
        list_id: Uuid,
        required: ListAccess,
    ) -> CoreResult<(Tx, ListGrant)> {
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        let grant = authorize(&mut *tx, user, list_id, required).await?;
        Ok((tx, grant))
    }
}

#[async_trait]
impl ShoppingListRepo for SqlitePool {
    async fn list_for_owner(&self, user_id: Uuid) -> CoreResult<Vec<ShoppingList>> {
        Ok(accessible_lists(&self.inner, user_id, None)
            .await?
            .into_iter()
            .filter(|a| a.access == ListAccess::Owner)
            .map(|a| a.list)
            .collect())
    }

    async fn shared_with(&self, user_id: Uuid) -> CoreResult<Vec<SharedList>> {
        Ok(accessible_lists(&self.inner, user_id, None)
            .await?
            .into_iter()
            .filter_map(|a| {
                let role = match a.access {
                    ListAccess::Owner => return None,
                    ListAccess::Edit => ShareRole::Edit,
                    ListAccess::View => ShareRole::View,
                };
                Some(SharedList {
                    list: a.list,
                    role,
                    owner_name: a.owner_name,
                })
            })
            .collect())
    }

    async fn create(&self, user_id: Uuid, name: &str) -> CoreResult<ShoppingList> {
        // A new list belongs to its creator; there is nothing to authorize.
        let name = clean_name("list", name)?;
        let row = sqlx::query(
            r"INSERT INTO shopping_lists (id, owner_id, name, created_at, updated_at)
              VALUES (?1, ?2, ?3, ?4, ?4)
              RETURNING id, owner_id, name, created_at, updated_at",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id.to_string())
        .bind(name)
        .bind(Utc::now())
        .fetch_one(&self.inner)
        .await?;

        row_to_list(&row)
    }

    async fn get(&self, user_id: Uuid, id: Uuid) -> CoreResult<ShoppingList> {
        let (tx, grant) = self.begin_read(user_id, id, ListAccess::View).await?;
        tx.commit().await?;
        Ok(grant.list)
    }

    async fn access(&self, user_id: Uuid, id: Uuid) -> CoreResult<ListAccess> {
        let (tx, grant) = self.begin_read(user_id, id, ListAccess::View).await?;
        tx.commit().await?;
        Ok(grant.access)
    }

    async fn rename(&self, user_id: Uuid, id: Uuid, name: &str) -> CoreResult<ShoppingList> {
        let name = clean_name("list", name)?;
        let (mut tx, grant) = self.begin_write(user_id, id, ListAccess::Owner).await?;
        let row = sqlx::query(
            r"UPDATE shopping_lists SET name = ?1, updated_at = ?2
                WHERE id = ?3
              RETURNING id, owner_id, name, created_at, updated_at",
        )
        .bind(name)
        .bind(Utc::now())
        .bind(grant.list_id())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;

        row_to_list(&row)
    }

    async fn delete(&self, user_id: Uuid, id: Uuid) -> CoreResult<()> {
        let (mut tx, grant) = self.begin_write(user_id, id, ListAccess::Owner).await?;
        sqlx::query(r"DELETE FROM shopping_lists WHERE id = ?1")
            .bind(grant.list_id())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn items(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        removed: bool,
    ) -> CoreResult<Vec<ListItem>> {
        // An unknown or foreign list is NotFound, not an empty list.
        let (mut tx, grant) = self.begin_read(user_id, list_id, ListAccess::View).await?;
        let items = fetch_items(&mut *tx, &grant, None, removed).await?;
        tx.commit().await?;
        Ok(items)
    }

    async fn add_item(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        text: &str,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem> {
        let text = clean_name("item", text)?;
        let id = Uuid::new_v4();
        let (mut tx, grant) = self.begin_write(user_id, list_id, ListAccess::Edit).await?;
        touch_list(&mut tx, &grant).await?;

        // Appended at the end; removed items keep their positions, so a
        // restored item returns to its place.
        sqlx::query(
            r"INSERT INTO list_items (id, list_id, text, quantity, position)
              VALUES (?1, ?2, ?3, ?4,
                      (SELECT COALESCE(MAX(position), -1) + 1
                         FROM list_items WHERE list_id = ?2))",
        )
        .bind(id.to_string())
        .bind(grant.list_id())
        .bind(text)
        .bind(clean_quantity(quantity))
        .execute(&mut *tx)
        .await?;
        let item = fetch_item(&mut *tx, &grant, id).await?;
        tx.commit().await?;
        Ok(item)
    }

    async fn add_product(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        product_id: Uuid,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem> {
        let id = Uuid::new_v4();
        // Owner only: the product comes from the owner's catalog.
        let (mut tx, grant) = self
            .begin_write(user_id, list_id, ListAccess::Owner)
            .await?;
        touch_list(&mut tx, &grant).await?;

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
        .bind(grant.list_id())
        .bind(clean_quantity(quantity))
        .bind(product_id.to_string())
        .bind(grant.owner_id())
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() == 0 {
            return Err(CoreError::Invalid("unknown product".to_owned()));
        }
        let item = fetch_item(&mut *tx, &grant, id).await?;
        tx.commit().await?;
        Ok(item)
    }

    async fn add_recipe(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        recipe_id: Uuid,
        multiplier: f64,
    ) -> CoreResult<Vec<ListItem>> {
        if !(multiplier.is_finite() && multiplier > 0.0 && multiplier <= 100.0) {
            return Err(CoreError::Invalid(
                "the multiplier must be more than 0 and at most 100".to_owned(),
            ));
        }
        let recipe = recipe_id.to_string();
        // Owner only: the recipe comes from the owner's catalog. The write
        // transaction is IMMEDIATE, so the ingredients read and the inserts
        // see one state, and a failure leaves no half-added recipe.
        let (mut tx, grant) = self
            .begin_write(user_id, list_id, ListAccess::Owner)
            .await?;
        let owner = grant.owner_id();
        let list = grant.list_id();
        touch_list(&mut tx, &grant).await?;

        let title: String =
            sqlx::query(r"SELECT title FROM recipes WHERE id = ?1 AND owner_id = ?2")
                .bind(&recipe)
                .bind(&owner)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(CoreError::NotFound)?
                .try_get("title")?;
        // The same owner-checked read as `RecipeRepo::get`: live product names
        // and categories, or the saved name once a product is gone.
        let ingredients = sqlx::query(INGREDIENT_SELECT)
            .bind(&recipe)
            .bind(&owner)
            .bind(None::<String>)
            .fetch_all(&mut *tx)
            .await?
            .iter()
            .map(|row| row_to_ingredient(row, grant.list.owner_id))
            .collect::<CoreResult<Vec<Ingredient>>>()?;
        if ingredients.is_empty() {
            return Err(CoreError::Invalid(format!("{title} has no ingredients")));
        }

        let first: i64 = sqlx::query(
            r"SELECT COALESCE(MAX(position), -1) + 1 AS next FROM list_items WHERE list_id = ?1",
        )
        .bind(&list)
        .fetch_one(&mut *tx)
        .await?
        .try_get("next")?;
        let unscaled = (multiplier - 1.0).abs() < f64::EPSILON;
        let mut ids = Vec::with_capacity(ingredients.len());
        for (offset, ingredient) in ingredients.into_iter().enumerate() {
            let position = first
                + i64::try_from(offset)
                    .map_err(|_| CoreError::Invalid("too many ingredients".to_owned()))?;
            let quantity = ingredient.quantity.map(|q| {
                if unscaled {
                    q
                } else {
                    quantity::scale(&q, multiplier)
                }
            });
            let id = Uuid::new_v4();
            sqlx::query(
                r"INSERT INTO list_items
                      (id, list_id, product_id, text, category_id, quantity, position)
                  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .bind(id.to_string())
            .bind(&list)
            .bind(ingredient.product_id.map(|p| p.to_string()))
            .bind(&ingredient.text)
            .bind(ingredient.category.map(|c| c.id.to_string()))
            .bind(quantity)
            .bind(position)
            .execute(&mut *tx)
            .await?;
            ids.push(id);
        }

        let mut added = Vec::with_capacity(ids.len());
        for id in ids {
            added.push(fetch_item(&mut *tx, &grant, id).await?);
        }
        tx.commit().await?;
        Ok(added)
    }

    async fn assign_category(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        item_id: Uuid,
        category_id: Uuid,
    ) -> CoreResult<ListItem> {
        // Owner only: the category comes from the owner's catalog.
        let (mut tx, grant) = self
            .begin_write(user_id, list_id, ListAccess::Owner)
            .await?;
        let list = grant.list_id();
        touch_list(&mut tx, &grant).await?;

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
        .bind(grant.owner_id())
        .bind(item_id.to_string())
        .bind(&list)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(unknown_category());
        }
        let item = fetch_item(&mut *tx, &grant, item_id).await?;
        tx.commit().await?;
        Ok(item)
    }

    async fn store_view(&self, user_id: Uuid, list_id: Uuid) -> CoreResult<Vec<ListGroup>> {
        // An unknown or foreign list is NotFound, not an empty view.
        let (mut tx, grant) = self.begin_read(user_id, list_id, ListAccess::View).await?;
        let rows = sqlx::query(STORE_VIEW_SELECT)
            .bind(grant.list_id())
            .bind(grant.owner_id())
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        let rows = rows
            .iter()
            .map(|row| -> CoreResult<(ListItem, Option<i64>)> {
                Ok((
                    row_to_item(row, grant.list.owner_id)?,
                    row.try_get("product_position")?,
                ))
            })
            .collect::<CoreResult<Vec<_>>>()?;
        Ok(group_for_store(rows))
    }

    async fn set_checked(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        item_id: Uuid,
        checked: bool,
    ) -> CoreResult<ListItem> {
        let (mut tx, grant) = self.begin_write(user_id, list_id, ListAccess::Edit).await?;
        touch_list(&mut tx, &grant).await?;

        let result = sqlx::query(
            r"UPDATE list_items SET checked = ?1
                WHERE id = ?2 AND list_id = ?3 AND removed_at IS NULL",
        )
        .bind(checked)
        .bind(item_id.to_string())
        .bind(grant.list_id())
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        let item = fetch_item(&mut *tx, &grant, item_id).await?;
        tx.commit().await?;
        Ok(item)
    }

    async fn remove_item(&self, user_id: Uuid, list_id: Uuid, item_id: Uuid) -> CoreResult<()> {
        let (mut tx, grant) = self.begin_write(user_id, list_id, ListAccess::Edit).await?;
        touch_list(&mut tx, &grant).await?;

        let result = sqlx::query(
            r"UPDATE list_items SET removed_at = ?1
                WHERE id = ?2 AND list_id = ?3 AND removed_at IS NULL",
        )
        .bind(Utc::now())
        .bind(item_id.to_string())
        .bind(grant.list_id())
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }

        tx.commit().await?;
        Ok(())
    }

    async fn restore_item(&self, user_id: Uuid, list_id: Uuid, item_id: Uuid) -> CoreResult<()> {
        let (mut tx, grant) = self.begin_write(user_id, list_id, ListAccess::Edit).await?;
        touch_list(&mut tx, &grant).await?;

        let result = sqlx::query(
            r"UPDATE list_items SET removed_at = NULL
                WHERE id = ?1 AND list_id = ?2 AND removed_at IS NOT NULL",
        )
        .bind(item_id.to_string())
        .bind(grant.list_id())
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }

        tx.commit().await?;
        Ok(())
    }

    async fn shares(&self, user_id: Uuid, list_id: Uuid) -> CoreResult<Vec<ListShare>> {
        let (mut tx, grant) = self.begin_read(user_id, list_id, ListAccess::Owner).await?;
        let rows = sqlx::query(SHARE_SELECT)
            .bind(grant.list_id())
            .bind(None::<String>)
            .fetch_all(&mut *tx)
            .await?;
        tx.commit().await?;
        rows.iter().map(row_to_share).collect()
    }

    async fn share(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        email: &str,
        role: ShareRole,
    ) -> CoreResult<Option<ListShare>> {
        let email = email.trim();
        if email.is_empty() {
            return Err(CoreError::Invalid("enter an email address".to_owned()));
        }
        let (mut tx, grant) = self
            .begin_write(user_id, list_id, ListAccess::Owner)
            .await?;

        // Emails are matched ignoring ASCII case (the owner's 2026-10-29
        // decision to treat emails case-insensitively), and only a single,
        // unambiguous account other than the owner gets the grant.
        let matches = sqlx::query(
            r"SELECT id FROM users WHERE lower(email) = lower(?1) AND id <> ?2 LIMIT 2",
        )
        .bind(email)
        .bind(grant.owner_id())
        .fetch_all(&mut *tx)
        .await?;
        let [grantee] = matches.as_slice() else {
            tx.commit().await?;
            return Ok(None);
        };
        let grantee: String = grantee.try_get("id")?;

        // Sharing again changes the role; the grant keeps its id and age.
        sqlx::query(
            r"INSERT INTO list_shares (id, list_id, grantee_id, role, created_at)
              VALUES (?1, ?2, ?3, ?4, ?5)
              ON CONFLICT (list_id, grantee_id) DO UPDATE SET role = excluded.role",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(grant.list_id())
        .bind(&grantee)
        .bind(role.as_str())
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query(
            r"SELECT s.id, s.list_id, s.grantee_id, u.display_name AS grantee_name,
                     u.email AS grantee_email, s.role, s.created_at
                FROM list_shares s
                JOIN users u ON u.id = s.grantee_id
               WHERE s.list_id = ?1 AND s.grantee_id = ?2",
        )
        .bind(grant.list_id())
        .bind(&grantee)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        row_to_share(&row).map(Some)
    }

    async fn revoke(&self, user_id: Uuid, list_id: Uuid, share_id: Uuid) -> CoreResult<ListShare> {
        let (mut tx, grant) = self
            .begin_write(user_id, list_id, ListAccess::Owner)
            .await?;
        let share = sqlx::query(SHARE_SELECT)
            .bind(grant.list_id())
            .bind(share_id.to_string())
            .fetch_optional(&mut *tx)
            .await?
            .as_ref()
            .map(row_to_share)
            .transpose()?
            .ok_or(CoreError::NotFound)?;
        sqlx::query(r"DELETE FROM list_shares WHERE id = ?1 AND list_id = ?2")
            .bind(share_id.to_string())
            .bind(grant.list_id())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(share)
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
