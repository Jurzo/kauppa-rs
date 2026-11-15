//! Sharing favorites and recipes for import (MB-0061, RR-0048).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Ingredient, ProductGroup, Recipe};

/// One share, as its owner sees it: who, and which switches are on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogShare {
    pub id: Uuid,
    pub grantee_id: Uuid,
    pub grantee_name: String,
    pub grantee_email: String,
    pub favorites: bool,
    pub recipes: bool,
    pub created_at: DateTime<Utc>,
}

/// A user who shares with the viewer, and what (RR-0048 sub-answer 3: the
/// display name only, never the email).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedCatalog {
    pub owner_id: Uuid,
    pub owner_name: String,
    pub favorites: bool,
    pub recipes: bool,
}

/// The sharer's favorites as the recipient may browse them, grouped by the
/// sharer's categories (uncategorized last).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedFavorites {
    pub owner_id: Uuid,
    pub owner_name: String,
    pub groups: Vec<ProductGroup>,
}

/// The sharer's recipes as the recipient may browse them, most recently
/// updated first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedRecipes {
    pub owner_id: Uuid,
    pub owner_name: String,
    pub recipes: Vec<Recipe>,
}

/// One shared recipe with its ingredients in order; each ingredient's
/// category is the sharer's (for matching by name, never linking).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedRecipe {
    pub owner_id: Uuid,
    pub owner_name: String,
    pub recipe: Recipe,
    pub ingredients: Vec<Ingredient>,
}
