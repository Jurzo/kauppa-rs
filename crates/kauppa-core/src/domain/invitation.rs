//! Invitations to create an account (MB-0023, RR-0012).
//!
//! An invitation is a bearer credential until it is used: whoever holds the
//! link can register once, with any email. Its state is derived from its
//! timestamps, never stored.

use crate::Invalid;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::domain::User;
use crate::error::{CoreError, CoreResult};
use crate::token::InviteToken;

/// How long a new invitation stays usable (MB-0023 `may_decide`: 1–14 days).
pub const INVITATION_TTL_DAYS: i64 = 7;

/// The longest note an admin may attach, in characters.
pub const NOTE_MAX_CHARS: usize = 200;

/// The lifetime of a new invitation.
#[must_use]
pub const fn invitation_ttl() -> Duration {
    Duration::days(INVITATION_TTL_DAYS)
}

/// Where an invitation is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InvitationState {
    /// Not used, not revoked, not expired: registering with it works.
    Pending,
    Used,
    Revoked,
    Expired,
}

impl InvitationState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Used => "used",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

/// The account an invitation created, for the admin portal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvitationUser {
    pub id: Uuid,
    pub display_name: String,
    pub email: String,
}

/// An invitation as stored, without its token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Invitation {
    pub id: Uuid,
    /// Free text from the admin ("for Mum"). Shown in the portal only, never
    /// logged.
    pub note: Option<String>,
    /// The admin who created it; `None` for the host command (or a deleted
    /// account).
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub used_at: Option<DateTime<Utc>>,
    /// The account created with it, while that account exists.
    pub used_by: Option<InvitationUser>,
}

impl Invitation {
    /// The state at `now`. Pending means exactly what registration checks:
    /// not used, not revoked and `expires_at > now`.
    #[must_use]
    pub fn state(&self, now: DateTime<Utc>) -> InvitationState {
        if self.used_at.is_some() {
            InvitationState::Used
        } else if self.revoked_at.is_some() {
            InvitationState::Revoked
        } else if self.expires_at <= now {
            InvitationState::Expired
        } else {
            InvitationState::Pending
        }
    }
}

/// A new invitation and its plaintext token, which exists only here.
#[derive(Debug)]
pub struct CreatedInvitation {
    pub invitation: Invitation,
    pub token: InviteToken,
}

/// An account created by using an invitation.
#[derive(Debug, Clone)]
pub struct Registered {
    pub user: User,
    pub invitation_id: Uuid,
}

/// Trim a note; blank means none. `Invalid` past [`NOTE_MAX_CHARS`].
pub fn clean_note(note: Option<&str>) -> CoreResult<Option<String>> {
    let Some(note) = note.map(str::trim).filter(|n| !n.is_empty()) else {
        return Ok(None);
    };
    if note.chars().count() > NOTE_MAX_CHARS {
        return Err(CoreError::Invalid(Invalid::NoteTooLong {
            max: NOTE_MAX_CHARS,
        }));
    }
    Ok(Some(note.to_owned()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn invitation() -> Invitation {
        Invitation {
            id: Uuid::nil(),
            note: None,
            created_by: None,
            created_at: at("2026-01-01T00:00:00Z"),
            expires_at: at("2026-01-08T00:00:00Z"),
            revoked_at: None,
            used_at: None,
            used_by: None,
        }
    }

    #[test]
    fn state_is_derived_from_timestamps() {
        let inv = invitation();
        assert_eq!(
            inv.state(at("2026-01-07T23:59:59Z")),
            InvitationState::Pending
        );
        assert_eq!(
            inv.state(at("2026-01-08T00:00:00Z")),
            InvitationState::Expired
        );
        let revoked = Invitation {
            revoked_at: Some(at("2026-01-02T00:00:00Z")),
            ..invitation()
        };
        assert_eq!(
            revoked.state(at("2026-01-03T00:00:00Z")),
            InvitationState::Revoked
        );
        let used = Invitation {
            used_at: Some(at("2026-01-02T00:00:00Z")),
            ..invitation()
        };
        // Used stays used after the expiry date, even if the account is gone.
        assert_eq!(
            used.state(at("2026-02-01T00:00:00Z")),
            InvitationState::Used
        );
    }

    #[test]
    fn notes_are_trimmed_and_capped() {
        assert_eq!(clean_note(None).unwrap(), None);
        assert_eq!(clean_note(Some("  ")).unwrap(), None);
        assert_eq!(clean_note(Some(" Äiti ")).unwrap().as_deref(), Some("Äiti"));
        let max = "ä".repeat(NOTE_MAX_CHARS);
        assert_eq!(clean_note(Some(&max)).unwrap(), Some(max.clone()));
        assert!(matches!(
            clean_note(Some(&format!("{max}x"))),
            Err(CoreError::Invalid(_))
        ));
    }
}
