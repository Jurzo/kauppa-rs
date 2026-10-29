use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A per-user category used to group products and shopping-list items.
///
/// Categories are owner-scoped and ordered by `position` (ascending).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub position: i64,
    pub created_at: DateTime<Utc>,
}
