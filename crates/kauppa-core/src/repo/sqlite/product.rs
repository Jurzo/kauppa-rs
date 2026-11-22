//! `ProductRepo` on SQLite: favorites and recipe-created products.

use crate::{Conflict, Entity, Invalid};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{
    Category, Direction, HiddenProductGroup, Product, ProductGroup, ProductUsage, ordering,
};
use crate::error::CoreError;
use crate::repo::ProductRepo;
use crate::repo::name_key::name_key;

use super::{
    CategoryRow, ProductRow, SqlitePool, clean_name, conflict_on_unique, parse_uuid,
    unknown_category,
};
use crate::export::{self, Candidate, ExportOutcome, ReviewAnswers, ReviewReason, Write};

/// A row's category columns, `group_id`, `group_name`, `group_position` and
/// `group_created_at` (all NULL when uncategorized), from a join that
/// checked the category's owner.
struct Group {
    id: Option<String>,
    name: Option<String>,
    position: Option<i64>,
    created_at: Option<DateTime<Utc>>,
}

impl Group {
    fn into_category(self, owner_id: Uuid) -> CoreResult<Option<Category>> {
        let Some(id) = self.id else { return Ok(None) };
        match (self.name, self.position, self.created_at) {
            (Some(name), Some(position), Some(created_at)) => Ok(Some(Category {
                id: parse_uuid(&id)?,
                owner_id,
                name,
                position,
                created_at,
            })),
            _ => Err(CoreError::Storage(
                "category group without its columns".to_owned(),
            )),
        }
    }
}

/// Group rows that are already sorted by category (uncategorized last) into
/// `(category, items)` runs.
fn group_by_category<T>(
    rows: impl IntoIterator<Item = (Group, CoreResult<T>)>,
    owner_id: Uuid,
) -> CoreResult<Vec<(Option<Category>, Vec<T>)>> {
    let mut groups: Vec<(Option<Category>, Vec<T>)> = Vec::new();
    for (group, item) in rows {
        let value = item?;
        let group_id = group.id.as_deref().map(parse_uuid).transpose()?;

        match groups.last_mut() {
            Some((category, items)) if category.as_ref().map(|c| c.id) == group_id => {
                items.push(value);
            }
            _ => groups.push((group.into_category(owner_id)?, vec![value])),
        }
    }
    Ok(groups)
}

/// `category`, if any, is the owner's (`NotFound` otherwise): the check
/// both order writes start with.
async fn check_group(
    tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    owner: &str,
    category: Option<&String>,
) -> CoreResult<()> {
    if let Some(category) = category {
        sqlx::query_scalar!(
            r#"SELECT 1 AS "found!: i64" FROM categories WHERE id = ?1 AND owner_id = ?2"#,
            category,
            owner
        )
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(CoreError::NotFound)?;
    }
    Ok(())
}

/// One favorites group's rows as stored (MB-0066): every product's id in
/// order, and the indices of the favorites among them. Hidden products keep
/// their indices when the favorites are reordered.
struct GroupSlots {
    ids: Vec<String>,
    slots: Vec<usize>,
}

impl GroupSlots {
    async fn load(
        tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
        owner: &str,
        category: Option<&String>,
    ) -> CoreResult<Self> {
        // `IS` so the uncategorized (NULL) group can be ordered too.
        let siblings = sqlx::query!(
            r#"SELECT id, is_favorite AS "is_favorite: bool" FROM products
                WHERE owner_id = ?1 AND category_id IS ?2
                ORDER BY position ASC, name ASC"#,
            owner,
            category
        )
        .fetch_all(&mut **tx)
        .await?;
        let mut ids = Vec::with_capacity(siblings.len());
        let mut slots = Vec::new();
        for (index, row) in siblings.into_iter().enumerate() {
            if row.is_favorite {
                slots.push(index);
            }
            ids.push(row.id);
        }
        Ok(Self { ids, slots })
    }

    /// Give the favorites' slots the `proposed` order, if it is exactly the
    /// favorites in some order (`ordering::is_reordering`); `false` leaves the
    /// group as it was.
    fn reorder(&mut self, proposed: Vec<String>) -> bool {
        let favorites: Vec<String> = self.slots.iter().map(|&i| self.ids[i].clone()).collect();
        if !ordering::is_reordering(&favorites, &proposed) {
            return false;
        }
        for (&slot, product_id) in self.slots.iter().zip(proposed) {
            self.ids[slot] = product_id;
        }
        true
    }

    /// Rewrite the whole group densely, which also heals any ties or gaps.
    async fn write(
        &self,
        tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
        owner: &str,
    ) -> CoreResult<()> {
        for (index, product_id) in self.ids.iter().enumerate() {
            let position = i64::try_from(index)
                .map_err(|_| CoreError::Invalid(Invalid::TooMany(Entity::Product)))?;
            sqlx::query!(
                r"UPDATE products SET position = ?1 WHERE id = ?2 AND owner_id = ?3",
                position,
                product_id,
                owner
            )
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl ProductRepo for SqlitePool {
    async fn create(&self, owner_id: Uuid, name: &str, category_id: Uuid) -> CoreResult<Product> {
        let name = clean_name(Entity::Product, name)?;
        let now = Utc::now();
        let owner = owner_id.to_string();
        let category = category_id.to_string();
        // IMMEDIATE: the hidden-product check and the insert can't race another
        // writer creating the same name.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;

        let category_owned = sqlx::query_scalar!(
            r#"SELECT 1 AS "found!: i64" FROM categories WHERE id = ?1 AND owner_id = ?2"#,
            category,
            owner
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if !category_owned {
            return Err(unknown_category());
        }

        // A hidden product (created from a recipe ingredient) of this name is
        // promoted rather than duplicated: one product, one row (RR-0001).
        let key = name_key(name);
        let promoted = sqlx::query_as!(
            ProductRow,
            r#"UPDATE products
                SET is_favorite = 1,
                    category_id = ?1,
                    position = (SELECT COALESCE(MAX(p.position), -1) + 1
                                  FROM products p
                                  WHERE p.owner_id = ?2 AND p.category_id = ?1),
                    updated_at = ?3
                WHERE owner_id = ?2 AND name_key = ?4 AND is_favorite = 0
              RETURNING id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                        created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>""#,
            category,
            owner,
            now,
            key
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(row) = promoted {
            let product = row.into_product()?;
            tx.commit().await?;
            return Ok(product);
        }

        // Nothing is inserted unless the category is the owner's, and the
        // product is appended at the end of that category.
        let new_id = Uuid::new_v4().to_string();
        let row = sqlx::query_as!(
            ProductRow,
            r#"INSERT INTO products
                  (id, owner_id, name, name_key, category_id, position, is_favorite,
                   created_at, updated_at)
              SELECT ?1, ?2, ?3, ?6, c.id,
                     (SELECT COALESCE(MAX(p.position), -1) + 1
                        FROM products p
                        WHERE p.owner_id = ?2 AND p.category_id = c.id),
                     1, ?4, ?4
                FROM categories c
                WHERE c.id = ?5 AND c.owner_id = ?2
              RETURNING id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                        created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>""#,
            new_id,
            owner,
            name,
            now,
            category,
            key
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            conflict_on_unique(e, || Conflict::Exists {
                entity: Entity::Product,
                name: name.to_owned(),
            })
        })?;

        let product = row
            .map(ProductRow::into_product)
            .transpose()?
            .ok_or_else(unknown_category)?;
        tx.commit().await?;
        Ok(product)
    }

    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ProductGroup>> {
        // The join also checks the category's owner, so a product can never be
        // shown under someone else's category; it would fall to uncategorized.
        let owner = owner_id.to_string();
        let rows = sqlx::query!(
            r#"SELECT p.id, p.owner_id, p.name, p.category_id, p.position, p.is_favorite AS "is_favorite: bool",
                     p.created_at AS "created_at: DateTime<Utc>", p.updated_at AS "updated_at: DateTime<Utc>",
                     c.id AS "group_id?", c.name AS "group_name?",
                     c.position AS "group_position?", c.created_at AS "group_created_at?: DateTime<Utc>"
                FROM products p
                LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = p.owner_id
                WHERE p.owner_id = ?1 AND p.is_favorite = 1
                ORDER BY c.id IS NULL, c.position, c.name, p.position, p.name"#,
            owner
        )
        .fetch_all(&self.inner)
        .await?;
        let rows = rows.into_iter().map(|r| {
            let group = Group {
                id: r.group_id,
                name: r.group_name,
                position: r.group_position,
                created_at: r.group_created_at,
            };
            let product = ProductRow {
                id: r.id,
                owner_id: r.owner_id,
                name: r.name,
                category_id: r.category_id,
                position: r.position,
                is_favorite: r.is_favorite,
                created_at: r.created_at,
                updated_at: r.updated_at,
            };
            (group, product.into_product())
        });

        Ok(group_by_category(rows, owner_id)?
            .into_iter()
            .map(|(category, products)| ProductGroup { category, products })
            .collect())
    }

    async fn find_by_name(&self, owner_id: Uuid, name: &str) -> CoreResult<Option<Product>> {
        let name = clean_name(Entity::Product, name)?;
        let owner = owner_id.to_string();
        let key = name_key(name);
        let row = sqlx::query_as!(
            ProductRow,
            r#"SELECT id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                     created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>"
                FROM products
                WHERE owner_id = ?1 AND name_key = ?2"#,
            owner,
            key
        )
        .fetch_optional(&self.inner)
        .await?;

        row.map(ProductRow::into_product).transpose()
    }

    async fn suggestion_names(&self, owner_id: Uuid) -> CoreResult<Vec<String>> {
        let owner = owner_id.to_string();
        Ok(sqlx::query_scalar!(
            r"SELECT name FROM products
                WHERE owner_id = ?1
                ORDER BY name_key, name",
            owner
        )
        .fetch_all(&self.inner)
        .await?)
    }

    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<Product> {
        let name = clean_name(Entity::Product, name)?;

        let now = Utc::now();
        let id_text = id.to_string();
        let owner = owner_id.to_string();
        let key = name_key(name);
        let row = sqlx::query_as!(
            ProductRow,
            r#"UPDATE products SET name = ?1, name_key = ?5, updated_at = ?2
                WHERE id = ?3 AND owner_id = ?4
              RETURNING id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                        created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>""#,
            name,
            now,
            id_text,
            owner,
            key
        )
        .fetch_optional(&self.inner)
        .await
        .map_err(|e| {
            conflict_on_unique(e, || Conflict::Exists {
                entity: Entity::Product,
                name: name.to_owned(),
            })
        })?;

        row.map(ProductRow::into_product)
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

        let product = sqlx::query_as!(
            ProductRow,
            r#"SELECT id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                     created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>"
                FROM products
                WHERE id = ?1 AND owner_id = ?2"#,
            id_str,
            owner
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CoreError::NotFound)?
        .into_product()?;
        if product.category_id == Some(category_id) {
            // Already there: keep its position rather than moving it to the end.
            return Ok(product);
        }

        let category_owned = sqlx::query_scalar!(
            r#"SELECT 1 AS "found!: i64" FROM categories WHERE id = ?1 AND owner_id = ?2"#,
            category,
            owner
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_some();
        if !category_owned {
            return Err(unknown_category());
        }

        let now = Utc::now();
        let product = sqlx::query_as!(
            ProductRow,
            r#"UPDATE products
                SET category_id = ?1,
                    position = (SELECT COALESCE(MAX(p.position), -1) + 1
                                  FROM products p
                                  WHERE p.owner_id = ?2 AND p.category_id = ?1),
                    updated_at = ?3
                WHERE id = ?4 AND owner_id = ?2
              RETURNING id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                        created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>""#,
            category,
            owner,
            now,
            id_str
        )
        .fetch_one(&mut *tx)
        .await?
        .into_product()?;

        tx.commit().await?;
        Ok(product)
    }

    async fn update(
        &self,
        owner_id: Uuid,
        id: Uuid,
        name: &str,
        category_id: Option<Uuid>,
    ) -> CoreResult<Product> {
        let name = clean_name(Entity::Product, name)?;
        let owner = owner_id.to_string();
        let id_str = id.to_string();
        // IMMEDIATE takes the write lock up front, so the reads below can't be
        // invalidated by another writer before the update.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;

        let current = sqlx::query_scalar!(
            r#"SELECT category_id FROM products WHERE id = ?1 AND owner_id = ?2"#,
            id_str,
            owner
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CoreError::NotFound)?;

        let category = category_id.map(|c| c.to_string());
        if let Some(category) = &category {
            let category_owned = sqlx::query_scalar!(
                r#"SELECT 1 AS "found!: i64" FROM categories WHERE id = ?1 AND owner_id = ?2"#,
                category,
                owner
            )
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
            if !category_owned {
                return Err(unknown_category());
            }
        }
        // A product that changes group goes to the end of its new one; one that
        // stays keeps its place.
        let moves = current != category;

        let now = Utc::now();
        let key = name_key(name);
        let product = sqlx::query_as!(
            ProductRow,
            r#"UPDATE products
                SET name = ?1,
                    name_key = ?2,
                    category_id = ?3,
                    position = CASE WHEN ?4
                        THEN (SELECT COALESCE(MAX(p.position), -1) + 1
                                FROM products p
                                WHERE p.owner_id = ?5 AND p.category_id IS ?3)
                        ELSE position END,
                    updated_at = ?6
                WHERE id = ?7 AND owner_id = ?5
              RETURNING id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                        created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>""#,
            name,
            key,
            category,
            moves,
            owner,
            now,
            id_str
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            conflict_on_unique(e, || Conflict::Exists {
                entity: Entity::Product,
                name: name.to_owned(),
            })
        })?
        .into_product()?;

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

        let category_id: Option<String> = sqlx::query_scalar!(
            r"SELECT category_id FROM products WHERE id = ?1 AND owner_id = ?2",
            id_str,
            owner
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CoreError::NotFound)?;

        // `IS` so the uncategorized (NULL) group can be ordered too.
        let siblings = sqlx::query!(
            r#"SELECT id, is_favorite AS "is_favorite: bool" FROM products
                WHERE owner_id = ?1 AND category_id IS ?2
                ORDER BY position ASC, name ASC"#,
            owner,
            category_id
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut ids = Vec::with_capacity(siblings.len());
        let mut favorite = Vec::with_capacity(siblings.len());
        for row in siblings {
            ids.push(row.id);
            favorite.push(row.is_favorite);
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
                .map_err(|_| CoreError::Invalid(Invalid::TooMany(Entity::Product)))?;
            sqlx::query!(
                r"UPDATE products SET position = ?1 WHERE id = ?2 AND owner_id = ?3",
                position,
                product_id,
                owner
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn set_favorite_order(
        &self,
        owner_id: Uuid,
        category_id: Option<Uuid>,
        ordered_ids: &[Uuid],
    ) -> CoreResult<bool> {
        let owner = owner_id.to_string();
        let category = category_id.map(|c| c.to_string());
        // IMMEDIATE: the set checked is the set written.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        check_group(&mut tx, &owner, category.as_ref()).await?;
        let mut group = GroupSlots::load(&mut tx, &owner, category.as_ref()).await?;
        let proposed: Vec<String> = ordered_ids.iter().map(Uuid::to_string).collect();
        if !group.reorder(proposed) {
            return Ok(false);
        }
        group.write(&mut tx, &owner).await?;
        tx.commit().await?;
        Ok(true)
    }

    async fn move_favorite_into_group(
        &self,
        owner_id: Uuid,
        id: Uuid,
        category_id: Option<Uuid>,
        ordered_ids: &[Uuid],
    ) -> CoreResult<bool> {
        let owner = owner_id.to_string();
        let id_str = id.to_string();
        let category = category_id.map(|c| c.to_string());
        // IMMEDIATE: the group and the product read are the ones written.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        check_group(&mut tx, &owner, category.as_ref()).await?;
        let moving = sqlx::query!(
            r#"SELECT category_id, is_favorite AS "is_favorite: bool" FROM products
                WHERE id = ?1 AND owner_id = ?2"#,
            id_str,
            owner
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CoreError::NotFound)?;
        // Only a favorite from another group moves; recipe-created products
        // stay in "From recipes" (MB-0071 P5).
        if !moving.is_favorite || moving.category_id == category {
            return Ok(false);
        }

        let mut group = GroupSlots::load(&mut tx, &owner, category.as_ref()).await?;
        // The moved favorite takes a new slot at the group's end; the order
        // then places every favorite, as a reorder does.
        group.ids.push(id_str.clone());
        group.slots.push(group.ids.len() - 1);
        let proposed: Vec<String> = ordered_ids.iter().map(Uuid::to_string).collect();
        if !group.reorder(proposed) {
            return Ok(false);
        }

        let now = Utc::now();
        sqlx::query!(
            r"UPDATE products SET category_id = ?1, updated_at = ?2 WHERE id = ?3 AND owner_id = ?4",
            category,
            now,
            id_str,
            owner
        )
        .execute(&mut *tx)
        .await?;
        group.write(&mut tx, &owner).await?;
        tx.commit().await?;
        Ok(true)
    }

    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()> {
        let owner = owner_id.to_string();
        let id_str = id.to_string();
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;

        // Ingredients show their product's live name and fall back to their
        // own `text` once unlinked, so save the current name there first: the
        // ingredients of a renamed product keep the new name (RR-0001
        // constraint 4). Only the owner's recipes, and only their product.
        sqlx::query!(
            r"UPDATE ingredients
                SET text = (SELECT p.name FROM products p
                              WHERE p.id = ?1 AND p.owner_id = ?2)
                WHERE product_id = ?1
                  AND recipe_id IN (SELECT r.id FROM recipes r WHERE r.owner_id = ?2)
                  AND EXISTS (SELECT 1 FROM products p WHERE p.id = ?1 AND p.owner_id = ?2)",
            id_str,
            owner
        )
        .execute(&mut *tx)
        .await?;

        // `ON DELETE SET NULL` unlinks the ingredients; nothing else goes.
        let result = sqlx::query!(
            r"DELETE FROM products WHERE id = ?1 AND owner_id = ?2",
            id_str,
            owner
        )
        .execute(&mut *tx)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        tx.commit().await?;
        Ok(())
    }

    async fn list_hidden_with_usage(&self, owner_id: Uuid) -> CoreResult<Vec<HiddenProductGroup>> {
        // Every join checks the owner (RR-0001 constraint 9): another user's
        // category never names a group, and only the owner's recipes count.
        let owner = owner_id.to_string();
        let rows = sqlx::query!(
            r#"SELECT p.id, p.owner_id, p.name, p.category_id, p.position, p.is_favorite AS "is_favorite: bool",
                     p.created_at AS "created_at: DateTime<Utc>", p.updated_at AS "updated_at: DateTime<Utc>",
                     c.id AS "group_id?", c.name AS "group_name?",
                     c.position AS "group_position?", c.created_at AS "group_created_at?: DateTime<Utc>",
                     (SELECT COUNT(DISTINCT r.id)
                        FROM ingredients i
                        JOIN recipes r ON r.id = i.recipe_id AND r.owner_id = p.owner_id
                        WHERE i.product_id = p.id) AS "recipe_count!: i64"
                FROM products p
                LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = p.owner_id
                WHERE p.owner_id = ?1 AND p.is_favorite = 0
                ORDER BY c.id IS NULL, c.position, c.name, p.name_key, p.name"#,
            owner
        )
        .fetch_all(&self.inner)
        .await?;
        let rows = rows.into_iter().map(|r| {
            let group = Group {
                id: r.group_id,
                name: r.group_name,
                position: r.group_position,
                created_at: r.group_created_at,
            };
            let product = ProductRow {
                id: r.id,
                owner_id: r.owner_id,
                name: r.name,
                category_id: r.category_id,
                position: r.position,
                is_favorite: r.is_favorite,
                created_at: r.created_at,
                updated_at: r.updated_at,
            };
            let usage = product.into_product().map(|product| ProductUsage {
                product,
                recipe_count: r.recipe_count,
            });
            (group, usage)
        });

        Ok(group_by_category(rows, owner_id)?
            .into_iter()
            .map(|(category, products)| HiddenProductGroup { category, products })
            .collect())
    }

    async fn make_favorite(&self, owner_id: Uuid, id: Uuid) -> CoreResult<Product> {
        let owner = owner_id.to_string();
        let id_str = id.to_string();
        // IMMEDIATE: the category check and the append can't race another
        // writer.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;

        // The category counts only if it is the owner's (RR-0001 constraint 9).
        let row = sqlx::query!(
            r#"SELECT p.id, p.owner_id, p.name, p.category_id, p.position, p.is_favorite AS "is_favorite: bool",
                     p.created_at AS "created_at: DateTime<Utc>", p.updated_at AS "updated_at: DateTime<Utc>", c.id AS "owned_category?"
                FROM products p
                LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = p.owner_id
                WHERE p.id = ?1 AND p.owner_id = ?2"#,
            id_str,
            owner
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CoreError::NotFound)?;
        let owned_category = row.owned_category;
        let product = ProductRow {
            id: row.id,
            owner_id: row.owner_id,
            name: row.name,
            category_id: row.category_id,
            position: row.position,
            is_favorite: row.is_favorite,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
        .into_product()?;
        if product.is_favorite {
            return Ok(product);
        }
        let Some(category) = owned_category else {
            return Err(CoreError::Invalid(Invalid::FavoriteNeedsCategory {
                name: product.name.clone(),
            }));
        };

        let now = Utc::now();
        let product = sqlx::query_as!(
            ProductRow,
            r#"UPDATE products
                SET is_favorite = 1,
                    position = (SELECT COALESCE(MAX(p.position), -1) + 1
                                  FROM products p
                                  WHERE p.owner_id = ?2 AND p.category_id = ?1),
                    updated_at = ?3
                WHERE id = ?4 AND owner_id = ?2 AND category_id = ?1
              RETURNING id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                        created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>""#,
            category,
            owner,
            now,
            id_str
        )
        .fetch_one(&mut *tx)
        .await?
        .into_product()?;

        tx.commit().await?;
        Ok(product)
    }

    async fn apply_export(
        &self,
        owner_id: Uuid,
        items: &[Candidate],
        review: Option<&ReviewAnswers>,
    ) -> CoreResult<ExportOutcome> {
        let owner = owner_id.to_string();
        // IMMEDIATE: the catalog the plan reads is the one the writes change,
        // so a confirmation can't apply answers to a catalog that changed
        // after its check (RR-0047 sub-answers 6 and 7). Only `products`,
        // and `categories` chosen on the review (RR-0052), are written, and
        // only the owner's rows.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        let categories = sqlx::query_as!(
            CategoryRow,
            r#"SELECT id, owner_id, name, position, created_at AS "created_at: DateTime<Utc>"
                FROM categories
                WHERE owner_id = ?1
                ORDER BY position ASC, name ASC"#,
            owner
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(CategoryRow::into_category)
        .collect::<CoreResult<Vec<Category>>>()?;
        let products = sqlx::query_as!(
            ProductRow,
            r#"SELECT id, owner_id, name, category_id, position, is_favorite AS "is_favorite: bool",
                      created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>"
                FROM products
                WHERE owner_id = ?1"#,
            owner
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(ProductRow::into_product)
        .collect::<CoreResult<Vec<Product>>>()?;

        let plan = export::plan(owner_id, items, &categories, &products);
        let decisions = match review {
            None if plan.needs_review() => Err(ReviewReason::Needed),
            None => Ok(export::Decisions::new()),
            Some(answers) => export::decide(&plan, &categories, answers)?,
        };
        let decisions = match decisions {
            Ok(decisions) => decisions,
            Err(reason) => {
                return Ok(ExportOutcome::Review {
                    plan,
                    categories,
                    reason,
                });
            }
        };

        // Chosen categories first, in this transaction (RR-0052 sub-answer
        // 4): a failure below rolls them back with everything else.
        let names = decisions.new_categories();
        let created = super::category::create_chosen(&mut tx, owner_id, &names).await?;
        let decisions = decisions.resolve(&created)?;
        let (writes, mut summary) = export::writes(&plan, &decisions);
        summary.categories_created = created.len();
        apply_writes(&mut tx, &owner, writes).await?;
        tx.commit().await?;
        Ok(ExportOutcome::Saved(summary))
    }
}

/// Apply an export's writes inside `tx`, for `owner` only (RR-0047
/// constraint 1). Each statement checks the category is the owner's (RR-0001
/// constraint 9); a miss is `Invalid` and the caller's transaction rolls
/// everything back.
async fn apply_writes(
    tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    owner: &str,
    writes: Vec<Write>,
) -> CoreResult<()> {
    let now = Utc::now();
    for write in writes {
        // Each statement checks the category is the owner's (RR-0001
        // constraint 9); a miss rolls everything back.
        let written = match write {
            Write::Insert { name, category } => {
                let name = clean_name(Entity::Product, &name)?.to_owned();
                let key = name_key(&name);
                let id = Uuid::new_v4().to_string();
                let category = category.to_string();
                sqlx::query!(
                    r#"INSERT INTO products
                              (id, owner_id, name, name_key, category_id, position, is_favorite,
                               created_at, updated_at)
                          SELECT ?1, ?2, ?3, ?4, c.id,
                                 (SELECT COALESCE(MAX(p.position), -1) + 1
                                    FROM products p
                                    WHERE p.owner_id = ?2 AND p.category_id = c.id),
                                 1, ?5, ?5
                            FROM categories c
                            WHERE c.id = ?6 AND c.owner_id = ?2"#,
                    id,
                    owner,
                    name,
                    key,
                    now,
                    category
                )
                .execute(&mut **tx)
                .await
                .map_err(|e| {
                    conflict_on_unique(e, || Conflict::Exists {
                        entity: Entity::Product,
                        name: name.clone(),
                    })
                })?
                .rows_affected()
            }
            Write::Favorite { id, category } => {
                let id = id.to_string();
                let category = category.to_string();
                // A product already in the category keeps its place only
                // if it was a favorite there; otherwise it goes last, as
                // `make_favorite` and `recategorize` do.
                sqlx::query!(
                        r#"UPDATE products
                            SET is_favorite = 1,
                                position = CASE WHEN is_favorite = 1 AND category_id IS ?1
                                                THEN position
                                                ELSE (SELECT COALESCE(MAX(p.position), -1) + 1
                                                        FROM products p
                                                        WHERE p.owner_id = ?2 AND p.category_id = ?1)
                                           END,
                                category_id = ?1,
                                updated_at = ?3
                            WHERE id = ?4 AND owner_id = ?2
                              AND EXISTS (SELECT 1 FROM categories c WHERE c.id = ?1 AND c.owner_id = ?2)"#,
                        category,
                        owner,
                        now,
                        id
                    )
                    .execute(&mut **tx)
                    .await?
                    .rows_affected()
            }
        };
        if written != 1 {
            return Err(unknown_category());
        }
    }
    Ok(())
}
