//! `InvitationRepo` and `AdminRepo` on SQLite (MB-0023, RR-0012).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::invitation::{clean_note, invitation_ttl};
use crate::domain::{CreatedInvitation, Invitation, InvitationUser, Registered};
use crate::error::CoreError;
use crate::repo::{AdminRepo, InvitationRepo};
use crate::token::InviteToken;

use super::user::insert_user;
use super::{SqlitePool, parse_uuid, whole_seconds};

/// Invitations with the account each created, if that still exists, newest
/// first. Binds: `?1` one invitation id, or NULL for all.
const INVITATION_SELECT: &str = r"
    SELECT i.id, i.note, i.created_by, i.created_at, i.expires_at, i.revoked_at, i.used_at,
           u.id AS used_by_id, u.display_name AS used_by_name, u.email AS used_by_email
      FROM invitations i
      LEFT JOIN users u ON u.id = i.used_by
     WHERE ?1 IS NULL OR i.id = ?1
     ORDER BY i.created_at DESC, i.id ASC";

fn row_to_invitation(row: &SqliteRow) -> CoreResult<Invitation> {
    let id: String = row.try_get("id")?;
    let created_by: Option<String> = row.try_get("created_by")?;
    let used_by_id: Option<String> = row.try_get("used_by_id")?;
    let used_by = used_by_id
        .as_deref()
        .map(|id| -> CoreResult<InvitationUser> {
            Ok(InvitationUser {
                id: parse_uuid(id)?,
                display_name: row.try_get("used_by_name")?,
                email: row.try_get("used_by_email")?,
            })
        })
        .transpose()?;
    Ok(Invitation {
        id: parse_uuid(&id)?,
        note: row.try_get("note")?,
        created_by: created_by.as_deref().map(parse_uuid).transpose()?,
        created_at: row.try_get("created_at")?,
        expires_at: row.try_get("expires_at")?,
        revoked_at: row.try_get("revoked_at")?,
        used_at: row.try_get("used_at")?,
        used_by,
    })
}

impl SqlitePool {
    async fn invitation(&self, id: Uuid) -> CoreResult<Invitation> {
        let row = sqlx::query(INVITATION_SELECT)
            .bind(Some(id.to_string()))
            .fetch_optional(&self.inner)
            .await?
            .ok_or(CoreError::NotFound)?;
        row_to_invitation(&row)
    }
}

#[async_trait]
impl InvitationRepo for SqlitePool {
    async fn create(
        &self,
        created_by: Option<Uuid>,
        note: Option<&str>,
        now: DateTime<Utc>,
    ) -> CoreResult<CreatedInvitation> {
        let note = clean_note(note)?;
        let token = InviteToken::generate()?;
        let id = Uuid::new_v4();
        let created_at = whole_seconds(now);
        let expires_at = created_at + invitation_ttl();
        sqlx::query(
            r"INSERT INTO invitations (id, token_hash, note, created_by, created_at, expires_at)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(id.to_string())
        .bind(token.hash())
        .bind(note.as_deref())
        .bind(created_by.map(|u| u.to_string()))
        .bind(created_at)
        .bind(expires_at)
        .execute(&self.inner)
        .await?;
        Ok(CreatedInvitation {
            invitation: Invitation {
                id,
                note,
                created_by,
                created_at,
                expires_at,
                revoked_at: None,
                used_at: None,
                used_by: None,
            },
            token,
        })
    }

    async fn list(&self) -> CoreResult<Vec<Invitation>> {
        let rows = sqlx::query(INVITATION_SELECT)
            .bind(None::<String>)
            .fetch_all(&self.inner)
            .await?;
        rows.iter().map(row_to_invitation).collect()
    }

    async fn revoke(&self, id: Uuid, now: DateTime<Utc>) -> CoreResult<Invitation> {
        let done = sqlx::query(
            r"UPDATE invitations SET revoked_at = ?2
               WHERE id = ?1 AND used_at IS NULL AND revoked_at IS NULL AND expires_at > ?2",
        )
        .bind(id.to_string())
        .bind(whole_seconds(now))
        .execute(&self.inner)
        .await?;
        if done.rows_affected() != 1 {
            return Err(CoreError::NotFound);
        }
        self.invitation(id).await
    }

    async fn register(
        &self,
        token_hash: &str,
        email: &str,
        display_name: &str,
        password_hash: &str,
        now: DateTime<Utc>,
    ) -> CoreResult<Option<Registered>> {
        let now = whole_seconds(now);
        let mut tx = self.inner.begin().await?;
        // 1. The user first: `used_by` references it. A duplicate email
        //    returns here, and dropping `tx` rolls back.
        let user = insert_user(&mut *tx, email, display_name, password_hash, now).await?;
        // 2. Use the invitation only if it is still pending. A concurrent
        //    registration waits for the write lock, then matches no row.
        let used_id: Option<String> = sqlx::query_scalar(
            r"UPDATE invitations SET used_at = ?1, used_by = ?2
               WHERE token_hash = ?3 AND used_at IS NULL AND revoked_at IS NULL
                 AND expires_at > ?1
           RETURNING id",
        )
        .bind(now)
        .bind(user.id.to_string())
        .bind(token_hash)
        .fetch_optional(&mut *tx)
        .await?;
        // 3. No pending invitation: undo the user, one answer for every reason.
        let Some(invitation_id) = used_id else {
            tx.rollback().await?;
            return Ok(None);
        };
        // 4.
        tx.commit().await?;
        Ok(Some(Registered {
            user,
            invitation_id: parse_uuid(&invitation_id)?,
        }))
    }
}

#[async_trait]
impl AdminRepo for SqlitePool {
    async fn is_admin(&self, user_id: Uuid) -> CoreResult<bool> {
        let found: Option<i64> = sqlx::query_scalar(r"SELECT 1 FROM admins WHERE user_id = ?1")
            .bind(user_id.to_string())
            .fetch_optional(&self.inner)
            .await?;
        Ok(found.is_some())
    }

    async fn grant(
        &self,
        user_id: Uuid,
        granted_by: Option<Uuid>,
        now: DateTime<Utc>,
    ) -> CoreResult<bool> {
        let done = sqlx::query(
            r"INSERT INTO admins (user_id, granted_at, granted_by) VALUES (?1, ?2, ?3)
              ON CONFLICT (user_id) DO NOTHING",
        )
        .bind(user_id.to_string())
        .bind(whole_seconds(now))
        .bind(granted_by.map(|u| u.to_string()))
        .execute(&self.inner)
        .await?;
        Ok(done.rows_affected() == 1)
    }
}
