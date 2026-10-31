use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::Category;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoppingList {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One item on a shopping list (MB-0004).
///
/// An item added from a catalog product copies the product's name and
/// category at add-time (RR-0001 constraint 6) and keeps them when the
/// catalog changes; `product_id` only records where it came from. A
/// free-text item has neither until the user picks a category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    pub id: Uuid,
    pub list_id: Uuid,
    pub product_id: Option<Uuid>,
    /// The name, as typed or as copied from the product.
    pub text: String,
    pub quantity: Option<String>,
    pub checked: bool,
    /// Position within the list (ascending).
    pub position: i64,
    /// The category the item belongs to; its name and order are live.
    /// `None` is uncategorized.
    pub category: Option<Category>,
    /// When the item was removed; `None` while it is on the list.
    pub removed_at: Option<DateTime<Utc>>,
}
