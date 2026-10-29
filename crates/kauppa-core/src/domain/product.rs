use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::Category;

/// A product in a user's catalog. Favorites are products with `is_favorite`.
///
/// `category_id` is `None` only when the product's category was deleted; new
/// products always get one (RR-0001: required at the handler layer, nullable
/// in the DB so category deletion can't destroy products).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub category_id: Option<Uuid>,
    /// Position within the product's category (ascending).
    pub position: i64,
    pub is_favorite: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The products of one category, in position order. `category` is `None` for
/// the uncategorized group, which always sorts last.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductGroup {
    pub category: Option<Category>,
    pub products: Vec<Product>,
}
