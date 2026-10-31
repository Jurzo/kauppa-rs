//! Persistence boundary.
//!
//! The web layer should depend on these traits, not on `sqlx` directly, so
//! domain tests can stub them and so backends can be swapped (SQLite today,
//! Postgres later) without touching handlers.

pub mod sqlite;

// Mission Brief acceptance tests for the repo traits; the module paths match
// the briefs' `cargo test -p kauppa-core repo::<name>::*` checks.
#[cfg(test)]
mod category;
#[cfg(test)]
mod product;

pub use sqlite::SqlitePool;

use async_trait::async_trait;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{Category, Direction, Product, ProductGroup, Recipe, ShoppingList, User};

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

/// The per-user product catalog (favorites). Every method is owner-scoped: a
/// product or category that isn't `owner_id`'s behaves as if it didn't exist.
#[async_trait]
pub trait ProductRepo: Send + Sync {
    /// Create a favorite in `category_id`, appended at the end of that
    /// category. Rejects empty names (`Invalid`), owner-duplicate names
    /// (`Conflict`), and a category the owner doesn't have (`Invalid`).
    async fn create(&self, owner_id: Uuid, name: &str, category_id: Uuid) -> CoreResult<Product>;

    /// The owner's products grouped by category: groups follow category order
    /// (uncategorized last), products follow `position` (name as tiebreak).
    /// Categories with no products are omitted.
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ProductGroup>>;

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

    /// Move a product one step within its category, atomically. Moving past
    /// either end is a no-op.
    async fn move_within_category(
        &self,
        owner_id: Uuid,
        id: Uuid,
        direction: Direction,
    ) -> CoreResult<()>;

    /// Delete a product the caller owns. `NotFound` if it isn't theirs.
    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()>;
}

#[async_trait]
pub trait ShoppingListRepo: Send + Sync {
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ShoppingList>>;
}

#[async_trait]
pub trait RecipeRepo: Send + Sync {
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Recipe>>;
}
