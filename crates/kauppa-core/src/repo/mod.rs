//! Persistence boundary.
//!
//! The web layer should depend on these traits, not on `sqlx` directly, so
//! domain tests can stub them and so backends can be swapped (SQLite today,
//! Postgres later) without touching handlers.

mod name_key;
pub mod sqlite;

// Mission Brief acceptance tests for the repo traits; the module paths match
// the briefs' `cargo test -p kauppa-core repo::<name>::*` checks.
#[cfg(test)]
mod category;
#[cfg(test)]
mod list;
#[cfg(test)]
mod product;
#[cfg(test)]
mod recipe;

pub use sqlite::SqlitePool;

use async_trait::async_trait;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{
    Category, Direction, Ingredient, ListItem, NewIngredient, Product, ProductGroup, Recipe,
    RecipeDetail, RecipeInput, ShoppingList, User,
};

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn find_by_email(&self, email: &str) -> CoreResult<Option<(User, String)>>;
    async fn create(
        &self,
        email: &str,
        display_name: &str,
        password_hash: &str,
    ) -> CoreResult<User>;
}

#[async_trait]
pub trait CategoryRepo: Send + Sync {
    /// Create a category for `owner_id`, appended at the end of their ordering.
    /// Rejects empty names (`Invalid`) and owner-duplicate names (`Conflict`).
    async fn create(&self, owner_id: Uuid, name: &str) -> CoreResult<Category>;

    /// The owner's categories, ordered by `position` ascending (name as tiebreak).
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Category>>;

    /// Rename a category the caller owns. `NotFound` if it isn't theirs.
    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<Category>;

    /// Persist a new ordering for the owner's categories atomically. Ids not
    /// owned by `owner_id` are ignored.
    async fn reorder(&self, owner_id: Uuid, ordered_ids: &[Uuid]) -> CoreResult<()>;

    /// Delete a category the caller owns. `NotFound` if it isn't theirs.
    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()>;
}

/// The per-user product catalog.
///
/// Favorites are the products with `is_favorite`; the rest were created from
/// recipe ingredients (MB-0003) and stay hidden from the favorites views.
/// Every method is owner-scoped: a product or category that isn't
/// `owner_id`'s behaves as if it didn't exist.
#[async_trait]
pub trait ProductRepo: Send + Sync {
    /// Create a favorite in `category_id`, appended at the end of that
    /// category. If a hidden product of the same name exists, it is promoted
    /// instead: it becomes a favorite and moves to `category_id`, keeping its
    /// id and name. Rejects empty names (`Invalid`), names of existing
    /// favorites (`Conflict`), and a category the owner doesn't have
    /// (`Invalid`).
    async fn create(&self, owner_id: Uuid, name: &str, category_id: Uuid) -> CoreResult<Product>;

    /// The owner's favorites grouped by category: groups follow category order
    /// (uncategorized last), products follow `position` (name as tiebreak).
    /// Hidden products and categories with no favorites are omitted.
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ProductGroup>>;

    /// The owner's product (favorite or hidden) whose name matches `name`
    /// ignoring case (RR-0003), if any. Rejects empty names (`Invalid`).
    async fn find_by_name(&self, owner_id: Uuid, name: &str) -> CoreResult<Option<Product>>;

    /// Rename a product the caller owns. `NotFound` if it isn't theirs.
    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<Product>;

    /// Move a product to another of the owner's categories, appended at its
    /// end. A no-op if it is already there. `Invalid` for a foreign category.
    async fn recategorize(
        &self,
        owner_id: Uuid,
        id: Uuid,
        category_id: Uuid,
    ) -> CoreResult<Product>;

    /// Move a product one step within its category, atomically, past the
    /// next favorite (hidden products keep their place). Moving past either
    /// end is a no-op.
    async fn move_within_category(
        &self,
        owner_id: Uuid,
        id: Uuid,
        direction: Direction,
    ) -> CoreResult<()>;

    /// Delete a product the caller owns. `NotFound` if it isn't theirs.
    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()>;
}

/// Named shopping lists (MB-0007).
///
/// Every method is owner-scoped: a list that isn't `owner_id`'s is
/// `NotFound`. (Shared lists, MB-0008, will widen this through RR-0002's
/// authorization predicate.)
#[async_trait]
pub trait ShoppingListRepo: Send + Sync {
    /// The owner's lists, most recently updated first.
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ShoppingList>>;

    /// Create an empty list. Rejects an empty name (`Invalid`); names need
    /// not be unique.
    async fn create(&self, owner_id: Uuid, name: &str) -> CoreResult<ShoppingList>;

    /// One of the owner's lists, to open and continue.
    async fn get(&self, owner_id: Uuid, id: Uuid) -> CoreResult<ShoppingList>;

    /// Rename a list; it counts as an update, so it moves to the top of the
    /// index. Rejects an empty name (`Invalid`).
    async fn rename(&self, owner_id: Uuid, id: Uuid, name: &str) -> CoreResult<ShoppingList>;

    /// Delete a list; its items go with it (`ON DELETE CASCADE`).
    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()>;

    /// A list's items in position order: the active ones by default
    /// (`removed_at IS NULL`, RR-0001 constraint 7), or only the removed ones.
    async fn items(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        removed: bool,
    ) -> CoreResult<Vec<ListItem>>;

    /// Append a free-text item. It is uncategorized until a category is
    /// assigned (RR-0001 constraint 5). Rejects empty text (`Invalid`).
    async fn add_item(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        text: &str,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem>;

    /// Append an item for one of the owner's products, copying its name and
    /// category now (RR-0001 constraint 6). `Invalid` for a product that
    /// isn't the owner's.
    async fn add_product(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        product_id: Uuid,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem>;

    /// Put an item in one of the owner's categories. Changes only this item,
    /// never the catalog. `Invalid` for a category that isn't the owner's.
    async fn assign_category(
        &self,
        owner_id: Uuid,
        list_id: Uuid,
        item_id: Uuid,
        category_id: Uuid,
    ) -> CoreResult<ListItem>;

    /// Soft-delete an active item (sets `removed_at`).
    async fn remove_item(&self, owner_id: Uuid, list_id: Uuid, item_id: Uuid) -> CoreResult<()>;

    /// Put a removed item back on the list, in its old position.
    async fn restore_item(&self, owner_id: Uuid, list_id: Uuid, item_id: Uuid) -> CoreResult<()>;
}

/// Recipes and their ingredients.
///
/// Every method is owner-scoped: a recipe that isn't `owner_id`'s is
/// `NotFound`, and so is an ingredient outside the given recipe.
#[async_trait]
pub trait RecipeRepo: Send + Sync {
    /// The owner's recipes, most recently updated first.
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Recipe>>;

    /// Create a recipe with no ingredients. Rejects an empty title and
    /// servings below 1 (`Invalid`).
    async fn create(&self, owner_id: Uuid, input: &RecipeInput) -> CoreResult<Recipe>;

    /// A recipe with its ingredients in position order.
    async fn get(&self, owner_id: Uuid, id: Uuid) -> CoreResult<RecipeDetail>;

    /// Replace a recipe's title, instructions, and servings. Validates like
    /// `create`.
    async fn update(&self, owner_id: Uuid, id: Uuid, input: &RecipeInput) -> CoreResult<Recipe>;

    /// Append an ingredient, atomically. Its product is the owner's product
    /// matching `product_name` ignoring case; if there is none, one is created
    /// in `category_id`, hidden from favorites. `Invalid` for an empty name, a
    /// new product without a category, or a category the owner doesn't have.
    ///
    /// Whether an existing product has a category is not checked here: RR-0001
    /// makes "an ingredient needs a categorized product" a handler rule.
    async fn add_ingredient(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient: &NewIngredient,
    ) -> CoreResult<Ingredient>;

    /// Set or clear (`None` or blank) an ingredient's quantity.
    async fn set_ingredient_quantity(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient_id: Uuid,
        quantity: Option<&str>,
    ) -> CoreResult<()>;

    /// Move an ingredient one step within its recipe, atomically. Moving past
    /// either end is a no-op.
    async fn move_ingredient(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient_id: Uuid,
        direction: Direction,
    ) -> CoreResult<()>;

    /// Remove an ingredient. Its product stays in the catalog.
    async fn remove_ingredient(
        &self,
        owner_id: Uuid,
        recipe_id: Uuid,
        ingredient_id: Uuid,
    ) -> CoreResult<()>;
}
