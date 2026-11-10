//! `CategoryRepo` on SQLite.

use crate::{Conflict, Entity, Invalid};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::Category;
use crate::error::CoreError;
use crate::repo::CategoryRepo;
use crate::repo::name_key::name_key;

use super::{SqlitePool, clean_name, row_to_category};

#[async_trait]
impl CategoryRepo for SqlitePool {
    async fn create(&self, owner_id: Uuid, name: &str) -> CoreResult<Category> {
        let name = clean_name(Entity::Category, name)?;
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
                CoreError::Conflict(Conflict::Exists {
                    entity: Entity::Category,
                    name: name.to_owned(),
                })
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
        let name = clean_name(Entity::Category, name)?;
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
                CoreError::Conflict(Conflict::Exists {
                    entity: Entity::Category,
                    name: name.to_owned(),
                })
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
                .map_err(|_| CoreError::Invalid(Invalid::TooMany(Entity::Category)))?;
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
