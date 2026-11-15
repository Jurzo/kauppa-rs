//! The predicate for reading another user's catalog (MB-0061, RR-0048
//! sub-answer 3): the only query that decides whether a user may read
//! someone's favorites or recipes, and the reads that need its grant.
//!
//! With `catalog_share.rs`, the only files that name `catalog_shares`;
//! `repo::share`'s guard test enforces it, and that every cross-user read
//! here takes a [`CatalogGrant`], which only [`authorize`] makes.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{Category, Ingredient, Product, ProductGroup, Recipe, SharedCatalog};
use crate::error::CoreError;

use super::parse_uuid;

/// What a share lets its grantee read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shared {
    Favorites,
    Recipes,
}

/// Proof that `owner`'s catalog is shared with the grantee for one kind.
/// Its fields are private to this file, so no other file can make one
/// (as `ListGrant`, RR-0036 constraint 2).
#[derive(Debug)]
pub(super) struct CatalogGrant {
    owner: Uuid,
    owner_name: String,
}

impl CatalogGrant {
    pub(super) const fn owner(&self) -> Uuid {
        self.owner
    }

    pub(super) fn owner_name(&self) -> &str {
        &self.owner_name
    }
}

/// Decide whether `user` may read `owner`'s `kind`. `NotFound` without a
/// share, or with that switch off: the same answer as a missing page, so a
/// share's existence doesn't leak (RR-0048 sub-answer 3).
pub(super) async fn authorize<'e, E>(
    exec: E,
    user: Uuid,
    owner: Uuid,
    kind: Shared,
) -> CoreResult<CatalogGrant>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let user_id = user.to_string();
    let owner_id = owner.to_string();
    let row = sqlx::query!(
        r#"SELECT s.favorites AS "favorites: bool", s.recipes AS "recipes: bool",
                  u.display_name
             FROM catalog_shares s
             JOIN users u ON u.id = s.owner_id
            WHERE s.owner_id = ?1 AND s.grantee_id = ?2"#,
        owner_id,
        user_id
    )
    .fetch_optional(exec)
    .await?
    .ok_or(CoreError::NotFound)?;
    let allowed = match kind {
        Shared::Favorites => row.favorites,
        Shared::Recipes => row.recipes,
    };
    if !allowed {
        return Err(CoreError::NotFound);
    }
    Ok(CatalogGrant {
        owner,
        owner_name: row.display_name,
    })
}

/// The users who share with `user`, and what. Only the grantee's own rows.
pub(super) async fn shared_with<'e, E>(exec: E, user: Uuid) -> CoreResult<Vec<SharedCatalog>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let user_id = user.to_string();
    sqlx::query!(
        r#"SELECT s.owner_id, u.display_name,
                  s.favorites AS "favorites: bool", s.recipes AS "recipes: bool"
             FROM catalog_shares s
             JOIN users u ON u.id = s.owner_id
            WHERE s.grantee_id = ?1
            ORDER BY u.display_name ASC, s.owner_id ASC"#,
        user_id
    )
    .fetch_all(exec)
    .await?
    .into_iter()
    .map(|row| {
        Ok(SharedCatalog {
            owner_id: parse_uuid(&row.owner_id)?,
            owner_name: row.display_name,
            favorites: row.favorites,
            recipes: row.recipes,
        })
    })
    .collect()
}

/// The granted owner's favorites, grouped by their categories in the owner's
/// order (uncategorized last), products by position.
pub(super) async fn shared_favorites<'e, E>(
    exec: E,
    grant: &CatalogGrant,
) -> CoreResult<Vec<ProductGroup>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let owner_id = grant.owner.to_string();
    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.position,
                  p.created_at AS "created_at: DateTime<Utc>",
                  p.updated_at AS "updated_at: DateTime<Utc>",
                  c.id AS "group_id?", c.name AS "group_name?",
                  c.position AS "group_position?",
                  c.created_at AS "group_created_at?: DateTime<Utc>"
             FROM products p
             LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = p.owner_id
            WHERE p.owner_id = ?1 AND p.is_favorite = 1
            ORDER BY c.id IS NULL, c.position ASC, c.name ASC, p.position ASC, p.name ASC"#,
        owner_id
    )
    .fetch_all(exec)
    .await?;
    let mut groups: Vec<ProductGroup> = Vec::new();
    for row in rows {
        let category = match (
            row.group_id,
            row.group_name,
            row.group_position,
            row.group_created_at,
        ) {
            (Some(id), Some(name), Some(position), Some(created_at)) => Some(Category {
                id: parse_uuid(&id)?,
                owner_id: grant.owner,
                name,
                position,
                created_at,
            }),
            _ => None,
        };
        let product = Product {
            id: parse_uuid(&row.id)?,
            owner_id: grant.owner,
            name: row.name,
            category_id: category.as_ref().map(|c| c.id),
            position: row.position,
            is_favorite: true,
            created_at: row.created_at,
            updated_at: row.updated_at,
        };
        let same_group = groups
            .last()
            .is_some_and(|g| g.category.as_ref().map(|c| c.id) == category.as_ref().map(|c| c.id));
        if same_group && let Some(group) = groups.last_mut() {
            group.products.push(product);
        } else {
            groups.push(ProductGroup {
                category,
                products: vec![product],
            });
        }
    }
    Ok(groups)
}

/// The granted owner's recipes, most recently updated first.
pub(super) async fn shared_recipes<'e, E>(exec: E, grant: &CatalogGrant) -> CoreResult<Vec<Recipe>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let owner_id = grant.owner.to_string();
    sqlx::query!(
        r#"SELECT id, title, instructions, servings,
                  created_at AS "created_at: DateTime<Utc>",
                  updated_at AS "updated_at: DateTime<Utc>"
             FROM recipes
            WHERE owner_id = ?1
            ORDER BY updated_at DESC, id ASC"#,
        owner_id
    )
    .fetch_all(exec)
    .await?
    .into_iter()
    .map(|row| {
        Ok(Recipe {
            id: parse_uuid(&row.id)?,
            owner_id: grant.owner,
            title: row.title,
            instructions: row.instructions,
            servings: row.servings,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    })
    .collect()
}

/// One of the granted owner's recipes; `NotFound` if it isn't theirs.
pub(super) async fn shared_recipe(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    grant: &CatalogGrant,
    recipe_id: Uuid,
) -> CoreResult<Recipe> {
    let owner_id = grant.owner.to_string();
    let id = recipe_id.to_string();
    let row = sqlx::query!(
        r#"SELECT id, title, instructions, servings,
                  created_at AS "created_at: DateTime<Utc>",
                  updated_at AS "updated_at: DateTime<Utc>"
             FROM recipes
            WHERE id = ?1 AND owner_id = ?2"#,
        id,
        owner_id
    )
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(CoreError::NotFound)?;
    Ok(Recipe {
        id: parse_uuid(&row.id)?,
        owner_id: grant.owner,
        title: row.title,
        instructions: row.instructions,
        servings: row.servings,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// A granted owner's recipe's ingredients in order, with their products'
/// current names and the owner's categories (every join checks the owner,
/// RR-0001 constraint 9).
pub(super) async fn shared_ingredients(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    grant: &CatalogGrant,
    recipe_id: Uuid,
) -> CoreResult<Vec<Ingredient>> {
    let owner_id = grant.owner.to_string();
    let id = recipe_id.to_string();
    sqlx::query!(
        r#"SELECT i.id, p.id AS "product_id?", COALESCE(p.name, i.text) AS "text!: String",
                  i.quantity, i.position,
                  c.id AS "category_id?", c.name AS "category_name?",
                  c.position AS "category_position?",
                  c.created_at AS "category_created_at?: DateTime<Utc>"
             FROM ingredients i
             JOIN recipes r ON r.id = i.recipe_id AND r.owner_id = ?2
             LEFT JOIN products p ON p.id = i.product_id AND p.owner_id = r.owner_id
             LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = r.owner_id
            WHERE i.recipe_id = ?1
            ORDER BY i.position ASC, i.id ASC"#,
        id,
        owner_id
    )
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|row| {
        let category = match (
            row.category_id,
            row.category_name,
            row.category_position,
            row.category_created_at,
        ) {
            (Some(id), Some(name), Some(position), Some(created_at)) => Some(Category {
                id: parse_uuid(&id)?,
                owner_id: grant.owner,
                name,
                position,
                created_at,
            }),
            _ => None,
        };
        Ok(Ingredient {
            id: parse_uuid(&row.id)?,
            recipe_id,
            product_id: row.product_id.as_deref().map(parse_uuid).transpose()?,
            text: row.text,
            quantity: row.quantity,
            position: row.position,
            category,
        })
    })
    .collect()
}
