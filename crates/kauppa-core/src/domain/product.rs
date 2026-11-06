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

/// A recipe-created product (MB-0014) with the number of the owner's recipes
/// that use it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductUsage {
    pub product: Product,
    /// Distinct recipes of the product's owner with an ingredient linked to
    /// it.
    pub recipe_count: i64,
}

/// The recipe-created products of one category, ordered by name. `category`
/// is `None` for the uncategorized group, which always sorts last.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenProductGroup {
    pub category: Option<Category>,
    pub products: Vec<ProductUsage>,
}
