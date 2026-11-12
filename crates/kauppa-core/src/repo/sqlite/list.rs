//! `ShoppingListRepo` on SQLite, with the central list access predicate
//! (RR-0002 constraint 1). This is the only repository file that names the
//! list tables (RR-0009 constraint 3); `repo::share`'s tests enforce it.

use crate::{Entity, Invalid};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{
    Ingredient, ListAccess, ListGroup, ListItem, ListShare, ShareRole, SharedList, ShoppingList,
    quantity,
};
use crate::error::CoreError;
use crate::repo::{ShoppingListRepo, name_key::name_key};

use super::list_rows::{group_for_store, parse_access, row_to_item, row_to_list, row_to_share};
use super::{
    INGREDIENT_SELECT, SqlitePool, clean_name, clean_quantity, row_to_ingredient, unknown_category,
};

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
        let name = clean_name(Entity::List, name)?;
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
        let name = clean_name(Entity::List, name)?;
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
        let text = clean_name(Entity::Item, text)?;
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
            return Err(CoreError::Invalid(Invalid::Unknown(Entity::Product)));
        }
        let item = fetch_item(&mut *tx, &grant, id).await?;
        tx.commit().await?;
        Ok(item)
    }

    async fn add_owner_favorite_by_name(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        text: &str,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem> {
        let text = clean_name(Entity::Item, text)?;
        let id = Uuid::new_v4();
        // `edit` is enough (RR-0024): only the list owner's favorites are read.
        let (mut tx, grant) = self.begin_write(user_id, list_id, ListAccess::Edit).await?;
        touch_list(&mut tx, &grant).await?;
        // The owner's (never the caller's) favorite with this `name_key`
        // (unique per owner) gives product, name and category as `add_product`
        // does; no match leaves `p` NULL: free text, as `add_item` adds it.
        sqlx::query(
            r"INSERT INTO list_items
                  (id, list_id, product_id, text, category_id, quantity, position)
              SELECT ?1, ?2, p.id, COALESCE(p.name, ?3), p.category_id, ?4,
                     (SELECT COALESCE(MAX(position), -1) + 1
                        FROM list_items WHERE list_id = ?2)
                FROM (SELECT 1) LEFT JOIN products p
                  ON p.owner_id = ?5 AND p.name_key = ?6 AND p.is_favorite = 1",
        )
        .bind(id.to_string())
        .bind(grant.list_id())
        .bind(text)
        .bind(clean_quantity(quantity))
        .bind(grant.owner_id())
        .bind(name_key(text))
        .execute(&mut *tx)
        .await?;
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
            return Err(CoreError::Invalid(Invalid::MultiplierRange));
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
            return Err(CoreError::Invalid(Invalid::NoIngredients {
                recipe: title.clone(),
            }));
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
                    .map_err(|_| CoreError::Invalid(Invalid::TooMany(Entity::Ingredient)))?;
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

    async fn set_item_quantity(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        item_id: Uuid,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem> {
        let (mut tx, grant) = self.begin_write(user_id, list_id, ListAccess::Edit).await?;
        touch_list(&mut tx, &grant).await?;

        let result = sqlx::query(
            r"UPDATE list_items SET quantity = ?1
                WHERE id = ?2 AND list_id = ?3 AND removed_at IS NULL",
        )
        .bind(clean_quantity(quantity))
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

    async fn update_item(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        item_id: Uuid,
        category_id: Option<Uuid>,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem> {
        // A category comes from the owner's catalog, so sending one needs
        // the owner, checked before it is looked up (RR-0033).
        let required = if category_id.is_some() {
            ListAccess::Owner
        } else {
            ListAccess::Edit
        };
        let (mut tx, grant) = self.begin_write(user_id, list_id, required).await?;
        let list = grant.list_id();
        touch_list(&mut tx, &grant).await?;

        let result = sqlx::query(
            r"UPDATE list_items SET quantity = ?1
                WHERE id = ?2 AND list_id = ?3 AND removed_at IS NULL",
        )
        .bind(clean_quantity(quantity))
        .bind(item_id.to_string())
        .bind(&list)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        if let Some(category_id) = category_id {
            // The list owner's category only (RR-0001 constraint 9). An
            // error drops `tx`, which rolls the quantity back too.
            let updated = sqlx::query(
                r"UPDATE list_items SET category_id = ?1
                    WHERE id = ?3 AND list_id = ?4 AND removed_at IS NULL
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
            return Err(CoreError::Invalid(Invalid::EmailRequired));
        }
        let (mut tx, grant) = self
            .begin_write(user_id, list_id, ListAccess::Owner)
            .await?;

        // Emails are matched ignoring ASCII case (RR-0028; the owner's
        // 2026-10-29 decision), and only a single, unambiguous account other
        // than the owner gets the grant.
        let matches = sqlx::query(
            r"SELECT id FROM users WHERE email = ?1 COLLATE NOCASE AND id <> ?2 LIMIT 2",
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
