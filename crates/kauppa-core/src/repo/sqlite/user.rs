//! `UserRepo` on SQLite.

use crate::Conflict;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{Language, User};
use crate::repo::UserRepo;
use crate::{CoreError, CoreResult};

use super::{SqlitePool, conflict_on_unique, parse_uuid};

#[async_trait]
impl UserRepo for SqlitePool {
    async fn find_by_email(&self, email: &str) -> CoreResult<Option<(User, String)>> {
        let row = sqlx::query!(
            r#"SELECT id, email, display_name, password_hash, created_at AS "created_at: DateTime<Utc>"
                FROM users
                WHERE email = ?1 COLLATE NOCASE"#,
            email
        )
        .fetch_optional(&self.inner)
        .await?;

        let Some(row) = row else { return Ok(None) };

        Ok(Some((
            User {
                id: parse_uuid(&row.id)?,
                email: row.email,
                display_name: row.display_name,
                created_at: row.created_at,
            },
            row.password_hash,
        )))
    }

    async fn create(
        &self,
        email: &str,
        display_name: &str,
        password_hash: &str,
    ) -> CoreResult<User> {
        insert_user(&self.inner, email, display_name, password_hash, Utc::now()).await
    }

    async fn language(&self, user_id: Uuid) -> CoreResult<Option<Language>> {
        let id = user_id.to_string();
        let stored: Option<Option<String>> =
            sqlx::query_scalar!(r"SELECT language FROM users WHERE id = ?1", id)
                .fetch_optional(&self.inner)
                .await?;
        Ok(stored.flatten().as_deref().and_then(Language::parse))
    }

    async fn set_language(&self, user_id: Uuid, language: Option<Language>) -> CoreResult<()> {
        let id = user_id.to_string();
        let tag = language.map(Language::tag);
        let done = sqlx::query!(r"UPDATE users SET language = ?2 WHERE id = ?1", id, tag)
            .execute(&self.inner)
            .await?;
        if done.rows_affected() == 0 {
            return Err(CoreError::NotFound);
        }
        Ok(())
    }
}

/// Insert a user row. Shared by [`UserRepo::create`] and the invitation
/// transaction (`InvitationRepo::register`). A duplicate email is
/// `Conflict`; its message never contains the email, so the WARN line
/// `WebError` logs can't leak it (RR-0012 sub-answer 4).
pub(super) async fn insert_user<'e, E>(
    executor: E,
    email: &str,
    display_name: &str,
    password_hash: &str,
    now: DateTime<Utc>,
) -> CoreResult<User>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let id = Uuid::new_v4();
    let id_text = id.to_string();
    sqlx::query!(
        r"INSERT INTO users (id, email, display_name, password_hash, created_at)
          VALUES (?1, ?2, ?3, ?4, ?5)",
        id_text,
        email,
        display_name,
        password_hash,
        now
    )
    .execute(executor)
    .await
    .map_err(|e| conflict_on_unique(e, || Conflict::EmailTaken))?;

    Ok(User {
        id,
        email: email.to_owned(),
        display_name: display_name.to_owned(),
        created_at: now,
    })
}
