//! `ProductRepo` on SQLite: favorites and recipe-created products.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{
    Category, Direction, HiddenProductGroup, Product, ProductGroup, ProductUsage, ordering,
};
use crate::error::CoreError;
use crate::repo::ProductRepo;
use crate::repo::name_key::name_key;

use super::{
    SqlitePool, clean_name, conflict_on_unique, parse_uuid, row_to_product, unknown_category,
};

/// Group rows that are already sorted by category (uncategorized last) into
/// `(category, items)` runs. Each row carries the category as `group_id`,
/// `group_name`, `group_position`, and `group_created_at` (all NULL when
/// uncategorized), from a join that checked the category's owner.
fn group_by_category<T>(
    rows: &[SqliteRow],
    owner_id: Uuid,
    item: impl Fn(&SqliteRow) -> CoreResult<T>,
) -> CoreResult<Vec<(Option<Category>, Vec<T>)>> {
    let mut groups: Vec<(Option<Category>, Vec<T>)> = Vec::new();
    for row in rows {
        let value = item(row)?;
        let group_id: Option<String> = row.try_get("group_id")?;
        let group_id = group_id.as_deref().map(parse_uuid).transpose()?;

        match groups.last_mut() {
            Some((category, items)) if category.as_ref().map(|c| c.id) == group_id => {
                items.push(value);
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
                groups.push((category, vec![value]));
            }
        }
    }
    Ok(groups)
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

        Ok(group_by_category(&rows, owner_id, row_to_product)?
            .into_iter()
            .map(|(category, products)| ProductGroup { category, products })
            .collect())
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

    async fn suggestion_names(&self, owner_id: Uuid) -> CoreResult<Vec<String>> {
        let rows = sqlx::query(
            r"SELECT name FROM products
                WHERE owner_id = ?1
                ORDER BY name_key, name",
        )
        .bind(owner_id.to_string())
        .fetch_all(&self.inner)
        .await?;

        rows.iter()
            .map(|row| row.try_get("name").map_err(CoreError::from))
            .collect()
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
        let owner = owner_id.to_string();
        let id_str = id.to_string();
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;

        // Ingredients show their product's live name and fall back to their
        // own `text` once unlinked, so save the current name there first: the
        // ingredients of a renamed product keep the new name (RR-0001
        // constraint 4). Only the owner's recipes, and only their product.
        sqlx::query(
            r"UPDATE ingredients
                SET text = (SELECT p.name FROM products p
                              WHERE p.id = ?1 AND p.owner_id = ?2)
                WHERE product_id = ?1
                  AND recipe_id IN (SELECT r.id FROM recipes r WHERE r.owner_id = ?2)
                  AND EXISTS (SELECT 1 FROM products p WHERE p.id = ?1 AND p.owner_id = ?2)",
        )
        .bind(&id_str)
        .bind(&owner)
        .execute(&mut *tx)
        .await?;

        // `ON DELETE SET NULL` unlinks the ingredients; nothing else goes.
        let result = sqlx::query(r"DELETE FROM products WHERE id = ?1 AND owner_id = ?2")
            .bind(&id_str)
            .bind(&owner)
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
        let rows = sqlx::query(
            r"SELECT p.id, p.owner_id, p.name, p.category_id, p.position, p.is_favorite,
                     p.created_at, p.updated_at,
                     c.id AS group_id, c.name AS group_name,
                     c.position AS group_position, c.created_at AS group_created_at,
                     (SELECT COUNT(DISTINCT r.id)
                        FROM ingredients i
                        JOIN recipes r ON r.id = i.recipe_id AND r.owner_id = p.owner_id
                        WHERE i.product_id = p.id) AS recipe_count
                FROM products p
                LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = p.owner_id
                WHERE p.owner_id = ?1 AND p.is_favorite = 0
                ORDER BY c.id IS NULL, c.position, c.name, p.name_key, p.name",
        )
        .bind(owner_id.to_string())
        .fetch_all(&self.inner)
        .await?;

        Ok(group_by_category(&rows, owner_id, |row| {
            Ok(ProductUsage {
                product: row_to_product(row)?,
                recipe_count: row.try_get("recipe_count")?,
            })
        })?
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
        let row = sqlx::query(
            r"SELECT p.id, p.owner_id, p.name, p.category_id, p.position, p.is_favorite,
                     p.created_at, p.updated_at, c.id AS owned_category
                FROM products p
                LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = p.owner_id
                WHERE p.id = ?1 AND p.owner_id = ?2",
        )
        .bind(&id_str)
        .bind(&owner)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(CoreError::NotFound)?;
        let product = row_to_product(&row)?;
        if product.is_favorite {
            return Ok(product);
        }
        let owned_category: Option<String> = row.try_get("owned_category")?;
        let Some(category) = owned_category else {
            return Err(CoreError::Invalid(format!(
                "choose a category for {} before making it a favorite",
                product.name
            )));
        };

        let row = sqlx::query(
            r"UPDATE products
                SET is_favorite = 1,
                    position = (SELECT COALESCE(MAX(p.position), -1) + 1
                                  FROM products p
                                  WHERE p.owner_id = ?2 AND p.category_id = ?1),
                    updated_at = ?3
                WHERE id = ?4 AND owner_id = ?2 AND category_id = ?1
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
}
