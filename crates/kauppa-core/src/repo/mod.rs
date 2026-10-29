//! Persistence boundary.
//!
//! The web layer should depend on these traits, not on `sqlx` directly, so
//! domain tests can stub them and so backends can be swapped (SQLite today,
//! Postgres later) without touching handlers.

pub mod sqlite;

pub use sqlite::SqlitePool;

use async_trait::async_trait;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{Category, Recipe, ShoppingList, User};

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

#[async_trait]
pub trait ShoppingListRepo: Send + Sync {
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<ShoppingList>>;
}

#[async_trait]
pub trait RecipeRepo: Send + Sync {
    async fn list_for_owner(&self, owner_id: Uuid) -> CoreResult<Vec<Recipe>>;
}
