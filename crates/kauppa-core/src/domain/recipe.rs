use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::Category;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub instructions: String,
    pub servings: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// One line of a recipe's ingredient list.
///
/// An ingredient references a catalog product (RR-0001), which carries its
/// category. `product_id` is `None` only when that product was deleted; the
/// ingredient then keeps the name it was saved with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ingredient {
    pub id: Uuid,
    pub recipe_id: Uuid,
    pub product_id: Option<Uuid>,
    /// The linked product's current name, or the saved name once unlinked.
    pub text: String,
    pub quantity: Option<String>,
    /// Position within the recipe (ascending).
    pub position: i64,
    /// The linked product's category; `None` if unlinked or uncategorized.
    pub category: Option<Category>,
}

/// A recipe with its ingredients in position order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeDetail {
    pub recipe: Recipe,
    pub ingredients: Vec<Ingredient>,
}

/// The editable fields of a recipe, as submitted by the user.
#[derive(Debug, Clone, Default)]
pub struct RecipeInput {
    pub title: String,
    pub instructions: String,
    pub servings: Option<i64>,
}

/// An ingredient to append to a recipe, named by product.
///
/// The product is matched by name (`name_key`, RR-0003). `category_id` is used
/// only when no product of that name exists yet: the new product is created in
/// it, hidden from favorites. An existing product keeps its own category.
#[derive(Debug, Clone, Default)]
pub struct NewIngredient {
    pub product_name: String,
    pub quantity: Option<String>,
    pub category_id: Option<Uuid>,
}
