//! `InvitationRepo` and `AdminRepo` on SQLite (MB-0023, RR-0012).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
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
/// first. Binds: `?1` one invitation id, or NULL for all. A macro, not a
/// constant, so both uses are checked at compile time (the query macros
/// take only a literal; MB-0049).
macro_rules! invitation_select {
    ($one:expr) => {
        sqlx::query_as!(
            InvitationRow,
            r#"
    SELECT i.id, i.note, i.created_by, i.created_at AS "created_at: DateTime<Utc>", i.expires_at AS "expires_at: DateTime<Utc>", i.revoked_at AS "revoked_at: DateTime<Utc>", i.used_at AS "used_at: DateTime<Utc>",
           u.id AS "used_by_id?", u.display_name AS "used_by_name?", u.email AS "used_by_email?"
      FROM invitations i
      LEFT JOIN users u ON u.id = i.used_by
     WHERE ?1 IS NULL OR i.id = ?1
     ORDER BY i.created_at DESC, i.id ASC"#,
            $one
        )
    };
}

/// A row of `invitation_select!`.
struct InvitationRow {
    id: String,
    note: Option<String>,
    created_by: Option<String>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    used_at: Option<DateTime<Utc>>,
    used_by_id: Option<String>,
    used_by_name: Option<String>,
    used_by_email: Option<String>,
}

impl InvitationRow {
    fn into_invitation(self) -> CoreResult<Invitation> {
        let used_by = match (self.used_by_id, self.used_by_name, self.used_by_email) {
            (Some(id), Some(display_name), Some(email)) => Some(InvitationUser {
                id: parse_uuid(&id)?,
                display_name,
                email,
            }),
            (None, _, _) => None,
            (Some(_), _, _) => {
                return Err(CoreError::Storage(
                    "invitation user without name or email".to_owned(),
                ));
            }
        };
        Ok(Invitation {
            id: parse_uuid(&self.id)?,
            note: self.note,
            created_by: self.created_by.as_deref().map(parse_uuid).transpose()?,
            created_at: self.created_at,
            expires_at: self.expires_at,
            revoked_at: self.revoked_at,
            used_at: self.used_at,
            used_by,
        })
    }
}

impl SqlitePool {
    async fn invitation(&self, id: Uuid) -> CoreResult<Invitation> {
        let id = Some(id.to_string());
        invitation_select!(id)
            .fetch_optional(&self.inner)
            .await?
            .ok_or(CoreError::NotFound)?
            .into_invitation()
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
        let id_text = id.to_string();
        let token_hash = token.hash();
        let note_text = note.as_deref();
        let creator = created_by.map(|u| u.to_string());
        sqlx::query!(
            r"INSERT INTO invitations (id, token_hash, note, created_by, created_at, expires_at)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            id_text,
            token_hash,
            note_text,
            creator,
            created_at,
            expires_at
        )
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
        let all: Option<String> = None;
        let rows = invitation_select!(all).fetch_all(&self.inner).await?;
        rows.into_iter()
            .map(InvitationRow::into_invitation)
            .collect()
    }

    async fn revoke(&self, id: Uuid, now: DateTime<Utc>) -> CoreResult<Invitation> {
        let id_text = id.to_string();
        let now = whole_seconds(now);
        let done = sqlx::query!(
            r"UPDATE invitations SET revoked_at = ?2
               WHERE id = ?1 AND used_at IS NULL AND revoked_at IS NULL AND expires_at > ?2",
            id_text,
            now
        )
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
        // IMMEDIATE: take the write lock up front (waiting out the busy
        // timeout), so the read below can't deadlock with a concurrent
        // registration when this transaction starts writing.
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        // 0. Only the holder of a pending invitation may learn that an email
        //    is taken: without this check, a duplicate email would fail
        //    step 1 before any invitation is looked at, for any token.
        let pending = sqlx::query_scalar!(
            r#"SELECT 1 AS "found!: i64" FROM invitations
               WHERE token_hash = ?1 AND used_at IS NULL AND revoked_at IS NULL
                 AND expires_at > ?2"#,
            token_hash,
            now
        )
        .fetch_optional(&mut *tx)
        .await?;
        if pending.is_none() {
            tx.rollback().await?;
            return Ok(None);
        }
        // 1. The user first: `used_by` references it. A duplicate email
        //    returns here, and dropping `tx` rolls back.
        let user = insert_user(&mut *tx, email, display_name, password_hash, now).await?;
        // 2. Use the invitation only if it is still pending. A concurrent
        //    registration waits for the write lock, then matches no row.
        let new_user = user.id.to_string();
        let used_id: Option<String> = sqlx::query_scalar!(
            r"UPDATE invitations SET used_at = ?1, used_by = ?2
               WHERE token_hash = ?3 AND used_at IS NULL AND revoked_at IS NULL
                 AND expires_at > ?1
           RETURNING id",
            now,
            new_user,
            token_hash
        )
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
        let id = user_id.to_string();
        let found = sqlx::query_scalar!(
            r#"SELECT 1 AS "found!: i64" FROM admins WHERE user_id = ?1"#,
            id
        )
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
        let id = user_id.to_string();
        let now = whole_seconds(now);
        let by = granted_by.map(|u| u.to_string());
        let done = sqlx::query!(
            r"INSERT INTO admins (user_id, granted_at, granted_by) VALUES (?1, ?2, ?3)
              ON CONFLICT (user_id) DO NOTHING",
            id,
            now,
            by
        )
        .execute(&self.inner)
        .await?;
        Ok(done.rows_affected() == 1)
    }
}
