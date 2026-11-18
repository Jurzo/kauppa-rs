//! `CategoryRepo` on SQLite.

use crate::{Conflict, Entity, Invalid};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{Category, ordering};
use crate::error::CoreError;
use crate::repo::CategoryRepo;
use crate::repo::name_key::name_key;

use super::{CategoryRow, SqlitePool, clean_name};

#[async_trait]
impl CategoryRepo for SqlitePool {
    async fn create(&self, owner_id: Uuid, name: &str) -> CoreResult<Category> {
        let mut conn = self.inner.acquire().await?;
        insert_category(&mut conn, owner_id, name).await
    }

    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Category>> {
        let owner = owner_id.to_string();
        let rows = sqlx::query_as!(
            CategoryRow,
            r#"SELECT id, owner_id, name, position, created_at AS "created_at: DateTime<Utc>"
                FROM categories
                WHERE owner_id = ?1
                ORDER BY position ASC, name ASC"#,
            owner
        )
        .fetch_all(&self.inner)
        .await?;

        rows.into_iter().map(CategoryRow::into_category).collect()
    }

    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<Category> {
        let name = clean_name(Entity::Category, name)?;
        let owner = owner_id.to_string();
        let id_str = id.to_string();

        let key = name_key(name);
        let result = sqlx::query!(
            r"UPDATE categories SET name = ?1, name_key = ?2 WHERE id = ?3 AND owner_id = ?4",
            name,
            key,
            id_str,
            owner
        )
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

        let row = sqlx::query_as!(
            CategoryRow,
            r#"SELECT id, owner_id, name, position, created_at AS "created_at: DateTime<Utc>"
                FROM categories
                WHERE id = ?1 AND owner_id = ?2"#,
            id_str,
            owner
        )
        .fetch_one(&self.inner)
        .await?;

        row.into_category()
    }

    async fn reorder(&self, owner_id: Uuid, ordered_ids: &[Uuid]) -> CoreResult<()> {
        let owner = owner_id.to_string();
        let mut tx = self.inner.begin().await?;

        for (index, id) in ordered_ids.iter().enumerate() {
            let position = i64::try_from(index)
                .map_err(|_| CoreError::Invalid(Invalid::TooMany(Entity::Category)))?;
            let id = id.to_string();
            sqlx::query!(
                r"UPDATE categories SET position = ?1 WHERE id = ?2 AND owner_id = ?3",
                position,
                id,
                owner
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn set_order(&self, owner_id: Uuid, ordered_ids: &[Uuid]) -> CoreResult<bool> {
        let owner = owner_id.to_string();
        // IMMEDIATE: the set checked is the set written.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        let current: Vec<String> =
            sqlx::query_scalar!(r"SELECT id FROM categories WHERE owner_id = ?1", owner)
                .fetch_all(&mut *tx)
                .await?;
        let proposed: Vec<String> = ordered_ids.iter().map(Uuid::to_string).collect();
        if !ordering::is_reordering(&current, &proposed) {
            return Ok(false);
        }
        for (index, id) in proposed.iter().enumerate() {
            let position = i64::try_from(index)
                .map_err(|_| CoreError::Invalid(Invalid::TooMany(Entity::Category)))?;
            sqlx::query!(
                r"UPDATE categories SET position = ?1 WHERE id = ?2 AND owner_id = ?3",
                position,
                id,
                owner
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
    }

    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()> {
        let owner = owner_id.to_string();
        let id = id.to_string();
        let result = sqlx::query!(
            r"DELETE FROM categories WHERE id = ?1 AND owner_id = ?2",
            id,
            owner
        )
        .execute(&self.inner)
        .await?;

        if result.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }
}

/// Insert a category for `owner_id`, last in their order: the one insert
/// path, used by "Add category" and, inside their transaction, by the
/// review's "Create ‹name›" (MB-0065, RR-0052 constraint 3). A name the
/// owner already has (ignoring case, RR-0003) is a `Conflict`.
pub(super) async fn insert_category(
    conn: &mut sqlx::SqliteConnection,
    owner_id: Uuid,
    name: &str,
) -> CoreResult<Category> {
    let name = clean_name(Entity::Category, name)?;
    let id = Uuid::new_v4();
    let id_str = id.to_string();
    let owner = owner_id.to_string();
    let now = Utc::now();

    // Append at the end of this owner's ordering.
    let position = sqlx::query_scalar!(
        r#"SELECT COALESCE(MAX(position), -1) + 1 AS "next!: i64"
                FROM categories
                WHERE owner_id = ?1"#,
        owner
    )
    .fetch_one(&mut *conn)
    .await?;

    let key = name_key(name);
    sqlx::query!(
        r"INSERT INTO categories (id, owner_id, name, name_key, position, created_at)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        id_str,
        owner,
        name,
        key,
        position,
        now
    )
    .execute(&mut *conn)
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

/// Create the review's chosen categories inside `tx`, in order, keyed by
/// `name_key` for [`crate::export::Decisions::resolve`] (RR-0052
/// sub-answer 4).
pub(super) async fn create_chosen(
    tx: &mut sqlx::Transaction<'static, sqlx::Sqlite>,
    owner_id: Uuid,
    names: &[String],
) -> CoreResult<std::collections::BTreeMap<String, Uuid>> {
    let mut created = std::collections::BTreeMap::new();
    for name in names {
        let category = insert_category(tx, owner_id, name).await?;
        created.insert(name_key(&category.name), category.id);
    }
    Ok(created)
}
