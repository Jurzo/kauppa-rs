//! Shared shopping lists (MB-0008, RR-0002).

use crate::Invalid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::CoreError;
use crate::domain::ShoppingList;

/// The role a share grants. `View` reads the list; `Edit` also adds, checks,
/// removes and restores items. Neither shares, revokes, renames or deletes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShareRole {
    View,
    Edit,
}

impl ShareRole {
    /// The stored form (`list_shares.role`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Edit => "edit",
        }
    }

    /// Parse the stored or submitted form. `Invalid` for anything else.
    pub fn parse(raw: &str) -> Result<Self, CoreError> {
        match raw.trim() {
            "view" => Ok(Self::View),
            "edit" => Ok(Self::Edit),
            _ => Err(CoreError::Invalid(Invalid::ShareRole)),
        }
    }
}

impl std::fmt::Display for ShareRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a user may do with a list, weakest first.
///
/// The outcome of the central access predicate (RR-0002 constraint 1). A user
/// with no access at all gets `NotFound` instead, so a list's existence never
/// leaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListAccess {
    /// A `view` grantee: read only.
    View,
    /// An `edit` grantee: add, check, remove and restore items.
    Edit,
    /// The owner: everything, including sharing, renaming and deleting.
    Owner,
}

impl From<ShareRole> for ListAccess {
    fn from(role: ShareRole) -> Self {
        match role {
            ShareRole::View => Self::View,
            ShareRole::Edit => Self::Edit,
        }
    }
}

/// One grant on a list, as its owner sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListShare {
    pub id: Uuid,
    pub list_id: Uuid,
    pub grantee_id: Uuid,
    pub grantee_name: String,
    pub grantee_email: String,
    pub role: ShareRole,
    pub created_at: DateTime<Utc>,
}

/// A list someone else shared with the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedList {
    pub list: ShoppingList,
    pub role: ShareRole,
    /// The owner's display name.
    pub owner_name: String,
}
