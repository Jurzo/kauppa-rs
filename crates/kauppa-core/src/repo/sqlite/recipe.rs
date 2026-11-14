//! `RecipeRepo` on SQLite.

use crate::{Conflict, Entity, Invalid};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{
    Direction, Ingredient, NewIngredient, Recipe, RecipeDetail, RecipeInput, ordering,
};
use crate::error::CoreError;
use crate::repo::RecipeRepo;
use crate::repo::name_key::name_key;

use super::{
    INGREDIENT_SELECT, SqlitePool, clean_name, clean_quantity, conflict_on_unique, parse_uuid,
    row_to_ingredient, unknown_category,
};

/// A `recipes` row as the query macros return it (`query_as!`).
struct RecipeRow {
    id: String,
    owner_id: String,
    title: String,
    instructions: String,
    servings: Option<i64>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl RecipeRow {
    fn into_recipe(self) -> CoreResult<Recipe> {
        Ok(Recipe {
            id: parse_uuid(&self.id)?,
            owner_id: parse_uuid(&self.owner_id)?,
            title: self.title,
            instructions: self.instructions,
            servings: self.servings,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// Trim a recipe's fields and check them: a non-empty title, servings ≥ 1.
fn clean_recipe(input: &RecipeInput) -> CoreResult<RecipeInput> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(CoreError::Invalid(Invalid::TitleEmpty));
    }
    if input.servings.is_some_and(|n| n < 1) {
        return Err(CoreError::Invalid(Invalid::ServingsTooFew));
    }
    Ok(RecipeInput {
        title: title.to_owned(),
        instructions: input.instructions.trim().to_owned(),
        servings: input.servings,
    })
}

impl SqlitePool {
    async fn fetch_ingredients(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        only: Option<Uuid>,
    ) -> CoreResult<Vec<Ingredient>> {
        // Runtime by decision (RR-0040), like `list.rs`, which shares the
        // constant: the macros would need aliases on it (LEFT JOIN
        // nullability, COALESCE, timestamps), and `list.rs` reads its columns
        // by name.
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
    let now = Utc::now();
    let result = sqlx::query!(
        r"UPDATE recipes SET updated_at = ?1 WHERE id = ?2 AND owner_id = ?3",
        now,
        recipe_id,
        owner
    )
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
        let rows = sqlx::query_as!(
            RecipeRow,
            r#"SELECT id, owner_id, title, instructions, servings, created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>"
                FROM recipes
                WHERE owner_id = ?1
                ORDER BY updated_at DESC"#,
            owner
        )
        .fetch_all(&self.inner)
        .await?;

        rows.into_iter().map(RecipeRow::into_recipe).collect()
    }

    async fn create(&self, owner_id: Uuid, input: &RecipeInput) -> CoreResult<Recipe> {
        let input = clean_recipe(input)?;
        let id = Uuid::new_v4().to_string();
        let owner = owner_id.to_string();
        let now = Utc::now();
        let row = sqlx::query_as!(
            RecipeRow,
            r#"INSERT INTO recipes
                  (id, owner_id, title, instructions, servings, created_at, updated_at)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
              RETURNING id, owner_id, title, instructions, servings, created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>""#,
            id,
            owner,
            input.title,
            input.instructions,
            input.servings,
            now
        )
        .fetch_one(&self.inner)
        .await?;

        row.into_recipe()
    }

    async fn get(&self, owner_id: Uuid, id: Uuid) -> CoreResult<RecipeDetail> {
        let id_text = id.to_string();
        let owner = owner_id.to_string();
        let row = sqlx::query_as!(
            RecipeRow,
            r#"SELECT id, owner_id, title, instructions, servings, created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>"
                FROM recipes
                WHERE id = ?1 AND owner_id = ?2"#,
            id_text,
            owner
        )
        .fetch_optional(&self.inner)
        .await?
        .ok_or(CoreError::NotFound)?;

        Ok(RecipeDetail {
            recipe: row.into_recipe()?,
            ingredients: self.fetch_ingredients(owner_id, id, None).await?,
        })
    }

    async fn update(&self, owner_id: Uuid, id: Uuid, input: &RecipeInput) -> CoreResult<Recipe> {
        let input = clean_recipe(input)?;
        let now = Utc::now();
        let id_text = id.to_string();
        let owner = owner_id.to_string();
        let row = sqlx::query_as!(
            RecipeRow,
            r#"UPDATE recipes
                SET title = ?1, instructions = ?2, servings = ?3, updated_at = ?4
                WHERE id = ?5 AND owner_id = ?6
              RETURNING id, owner_id, title, instructions, servings, created_at AS "created_at: DateTime<Utc>", updated_at AS "updated_at: DateTime<Utc>""#,
            input.title,
            input.instructions,
            input.servings,
            now,
            id_text,
            owner
        )
        .fetch_optional(&self.inner)
        .await?;

        row.map(RecipeRow::into_recipe)
            .transpose()?
            .ok_or(CoreError::NotFound)
    }

    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()> {
        // `ingredients.recipe_id` cascades (0001); `ingredients.product_id`
        // points the other way, so no product is touched.
        let id_text = id.to_string();
        let owner = owner_id.to_string();
        let result = sqlx::query!(
            r"DELETE FROM recipes WHERE id = ?1 AND owner_id = ?2",
            id_text,
            owner
        )
        .execute(&self.inner)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }

    async fn add_ingredient(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient: &NewIngredient,
    ) -> CoreResult<Ingredient> {
        let name = clean_name(Entity::Product, &ingredient.product_name)?;
        let owner = owner_id.to_string();
        let recipe = recipe_id.to_string();
        let now = Utc::now();
        // IMMEDIATE: the product lookup and its creation can't race another
        // writer adding the same name.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        touch_owned_recipe(&mut tx, &owner, &recipe).await?;

        // Reuse the owner's product of this name (MB-0003 P3, matched through
        // name_key per RR-0003 constraint 3); its category is kept as is.
        let key = name_key(name);
        let existing = sqlx::query!(
            r"SELECT id, name FROM products WHERE owner_id = ?1 AND name_key = ?2",
            owner,
            key
        )
        .fetch_optional(&mut *tx)
        .await?;
        let (product_id, product_name): (String, String) = if let Some(row) = existing {
            (row.id, row.name)
        } else {
            let category = ingredient.category_id.ok_or_else(|| {
                CoreError::Invalid(Invalid::NewProductNeedsCategory {
                    name: name.to_owned(),
                })
            })?;
            // A new product, hidden from favorites, appended at the end of an
            // owned category; nothing is inserted for a foreign one.
            let new_id = Uuid::new_v4().to_string();
            let category = category.to_string();
            let created: Option<String> = sqlx::query_scalar!(
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
                new_id,
                owner,
                name,
                key,
                now,
                category
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                conflict_on_unique(e, || Conflict::Exists {
                    entity: Entity::Product,
                    name: name.to_owned(),
                })
            })?;
            (created.ok_or_else(unknown_category)?, name.to_owned())
        };

        let id = Uuid::new_v4();
        let id_text = id.to_string();
        let quantity = clean_quantity(ingredient.quantity.as_deref());
        sqlx::query!(
            r"INSERT INTO ingredients (id, recipe_id, product_id, text, quantity, position)
              VALUES (?1, ?2, ?3, ?4, ?5,
                      (SELECT COALESCE(MAX(position), -1) + 1
                         FROM ingredients WHERE recipe_id = ?2))",
            id_text,
            recipe,
            product_id,
            product_name,
            quantity
        )
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

        let quantity = clean_quantity(quantity);
        let id = ingredient_id.to_string();
        let result = sqlx::query!(
            r"UPDATE ingredients SET quantity = ?1 WHERE id = ?2 AND recipe_id = ?3",
            quantity,
            id,
            recipe
        )
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

        let mut ids: Vec<String> = sqlx::query_scalar!(
            r"SELECT id FROM ingredients WHERE recipe_id = ?1 ORDER BY position ASC, id ASC",
            recipe
        )
        .fetch_all(&mut *tx)
        .await?;

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
                .map_err(|_| CoreError::Invalid(Invalid::TooMany(Entity::Ingredient)))?;
            sqlx::query!(
                r"UPDATE ingredients SET position = ?1 WHERE id = ?2 AND recipe_id = ?3",
                position,
                id,
                recipe
            )
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

        let id = ingredient_id.to_string();
        let result = sqlx::query!(
            r"DELETE FROM ingredients WHERE id = ?1 AND recipe_id = ?2",
            id,
            recipe
        )
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }

        tx.commit().await?;
        Ok(())
    }
}
