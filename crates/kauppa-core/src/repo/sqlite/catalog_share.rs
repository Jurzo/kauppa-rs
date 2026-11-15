//! `CatalogShareRepo` on SQLite (MB-0061, RR-0048): the caller's own shares,
//! and reads of another user's catalog through `catalog_access.rs`'s grant.
//! With that file, the only files that name `catalog_shares`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{CatalogShare, SharedCatalog, SharedFavorites, SharedRecipe, SharedRecipes};
use crate::error::CoreError;
use crate::repo::CatalogShareRepo;
use crate::{CoreResult, Invalid};

use super::catalog_access::{self, Shared};
use super::{SqlitePool, parse_uuid};

/// A share row with its grantee, as `query_as!` returns it.
struct ShareRow {
    id: String,
    grantee_id: String,
    grantee_name: String,
    grantee_email: String,
    favorites: bool,
    recipes: bool,
    created_at: DateTime<Utc>,
}

impl ShareRow {
    fn into_share(self) -> CoreResult<CatalogShare> {
        Ok(CatalogShare {
            id: parse_uuid(&self.id)?,
            grantee_id: parse_uuid(&self.grantee_id)?,
            grantee_name: self.grantee_name,
            grantee_email: self.grantee_email,
            favorites: self.favorites,
            recipes: self.recipes,
            created_at: self.created_at,
        })
    }
}

impl SqlitePool {
    /// One of `owner`'s shares, by id, inside `tx`.
    async fn own_share(
        tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
        owner: &str,
        share: &str,
    ) -> CoreResult<Option<CatalogShare>> {
        sqlx::query_as!(
            ShareRow,
            r#"SELECT s.id, s.grantee_id, u.display_name AS grantee_name,
                      u.email AS grantee_email,
                      s.favorites AS "favorites: bool", s.recipes AS "recipes: bool",
                      s.created_at AS "created_at: DateTime<Utc>"
                 FROM catalog_shares s
                 JOIN users u ON u.id = s.grantee_id
                WHERE s.id = ?1 AND s.owner_id = ?2"#,
            share,
            owner
        )
        .fetch_optional(&mut **tx)
        .await?
        .map(ShareRow::into_share)
        .transpose()
    }
}

#[async_trait]
impl CatalogShareRepo for SqlitePool {
    async fn catalog_shares(&self, owner_id: Uuid) -> CoreResult<Vec<CatalogShare>> {
        let owner = owner_id.to_string();
        sqlx::query_as!(
            ShareRow,
            r#"SELECT s.id, s.grantee_id, u.display_name AS grantee_name,
                      u.email AS grantee_email,
                      s.favorites AS "favorites: bool", s.recipes AS "recipes: bool",
                      s.created_at AS "created_at: DateTime<Utc>"
                 FROM catalog_shares s
                 JOIN users u ON u.id = s.grantee_id
                WHERE s.owner_id = ?1
                ORDER BY s.created_at ASC, s.id ASC"#,
            owner
        )
        .fetch_all(&self.inner)
        .await?
        .into_iter()
        .map(ShareRow::into_share)
        .collect()
    }

    async fn share_catalog(
        &self,
        owner_id: Uuid,
        email: &str,
        favorites: bool,
        recipes: bool,
    ) -> CoreResult<Option<CatalogShare>> {
        let email = email.trim();
        if email.is_empty() {
            return Err(CoreError::Invalid(Invalid::EmailRequired));
        }
        let owner = owner_id.to_string();
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        // Exactly one account other than the owner, ignoring ASCII case
        // (RR-0028); anything else shares nothing and answers the same.
        let matches = sqlx::query_scalar!(
            r"SELECT id FROM users WHERE email = ?1 COLLATE NOCASE AND id <> ?2 LIMIT 2",
            email,
            owner
        )
        .fetch_all(&mut *tx)
        .await?;
        let [grantee] = matches.as_slice() else {
            tx.commit().await?;
            return Ok(None);
        };
        if !favorites && !recipes {
            sqlx::query!(
                r"DELETE FROM catalog_shares WHERE owner_id = ?1 AND grantee_id = ?2",
                owner,
                grantee
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(None);
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        // Sharing again changes the switches; the share keeps its id and age.
        sqlx::query!(
            r"INSERT INTO catalog_shares (id, owner_id, grantee_id, favorites, recipes, created_at)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6)
              ON CONFLICT (owner_id, grantee_id)
              DO UPDATE SET favorites = excluded.favorites, recipes = excluded.recipes",
            id,
            owner,
            grantee,
            favorites,
            recipes,
            now
        )
        .execute(&mut *tx)
        .await?;
        let share_id = sqlx::query_scalar!(
            r"SELECT id FROM catalog_shares WHERE owner_id = ?1 AND grantee_id = ?2",
            owner,
            grantee
        )
        .fetch_one(&mut *tx)
        .await?;
        let share = Self::own_share(&mut tx, &owner, &share_id).await?;
        tx.commit().await?;
        Ok(share)
    }

    async fn set_catalog_share(
        &self,
        owner_id: Uuid,
        share_id: Uuid,
        favorites: bool,
        recipes: bool,
    ) -> CoreResult<Option<CatalogShare>> {
        let owner = owner_id.to_string();
        let share = share_id.to_string();
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        if Self::own_share(&mut tx, &owner, &share).await?.is_none() {
            return Err(CoreError::NotFound);
        }
        if !favorites && !recipes {
            sqlx::query!(
                r"DELETE FROM catalog_shares WHERE id = ?1 AND owner_id = ?2",
                share,
                owner
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(None);
        }
        sqlx::query!(
            r"UPDATE catalog_shares SET favorites = ?1, recipes = ?2 WHERE id = ?3 AND owner_id = ?4",
            favorites,
            recipes,
            share,
            owner
        )
        .execute(&mut *tx)
        .await?;
        let updated = Self::own_share(&mut tx, &owner, &share).await?;
        tx.commit().await?;
        Ok(updated)
    }

    async fn revoke_catalog_share(&self, owner_id: Uuid, share_id: Uuid) -> CoreResult<()> {
        let owner = owner_id.to_string();
        let share = share_id.to_string();
        let deleted = sqlx::query!(
            r"DELETE FROM catalog_shares WHERE id = ?1 AND owner_id = ?2",
            share,
            owner
        )
        .execute(&self.inner)
        .await?;
        if deleted.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }

    async fn shared_with(&self, user_id: Uuid) -> CoreResult<Vec<SharedCatalog>> {
        catalog_access::shared_with(&self.inner, user_id).await
    }

    async fn shared_favorites(&self, user_id: Uuid, owner_id: Uuid) -> CoreResult<SharedFavorites> {
        // One read transaction: the grant and the read see one state.
        let mut tx = self.inner.begin().await?;
        let grant =
            catalog_access::authorize(&mut *tx, user_id, owner_id, Shared::Favorites).await?;
        let groups = catalog_access::shared_favorites(&mut *tx, &grant).await?;
        tx.commit().await?;
        Ok(SharedFavorites {
            owner_id: grant.owner(),
            owner_name: grant.owner_name().to_owned(),
            groups,
        })
    }

    async fn shared_recipes(&self, user_id: Uuid, owner_id: Uuid) -> CoreResult<SharedRecipes> {
        let mut tx = self.inner.begin().await?;
        let grant = catalog_access::authorize(&mut *tx, user_id, owner_id, Shared::Recipes).await?;
        let recipes = catalog_access::shared_recipes(&mut *tx, &grant).await?;
        tx.commit().await?;
        Ok(SharedRecipes {
            owner_id: grant.owner(),
            owner_name: grant.owner_name().to_owned(),
            recipes,
        })
    }

    async fn shared_recipe(
        &self,
        user_id: Uuid,
        owner_id: Uuid,
        recipe_id: Uuid,
    ) -> CoreResult<SharedRecipe> {
        let mut tx = self.inner.begin().await?;
        let grant = catalog_access::authorize(&mut *tx, user_id, owner_id, Shared::Recipes).await?;
        let recipe = catalog_access::shared_recipe(&mut tx, &grant, recipe_id).await?;
        let ingredients = catalog_access::shared_ingredients(&mut tx, &grant, recipe_id).await?;
        tx.commit().await?;
        Ok(SharedRecipe {
            owner_id: grant.owner(),
            owner_name: grant.owner_name().to_owned(),
            recipe,
            ingredients,
        })
    }
}
