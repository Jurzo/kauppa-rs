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
#[cfg(test)]
mod share;

pub use sqlite::SqlitePool;

use async_trait::async_trait;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{
    Category, Direction, HiddenProductGroup, Ingredient, ListAccess, ListGroup, ListItem,
    ListShare, NewIngredient, Product, ProductGroup, Recipe, RecipeDetail, RecipeInput, ShareRole,
    SharedList, ShoppingList, User,
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

    /// The names of all the owner's products, favorites and hidden alike, for
    /// suggesting while typing (MB-0013). Sorted by name ignoring case: by the
    /// `name_key` (RR-0003), with the stored name as tiebreak.
    async fn suggestion_names(&self, owner_id: Uuid) -> CoreResult<Vec<String>>;

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
    /// Recipe ingredients that use it stay, unlinked, under the product's
    /// current name (RR-0001 constraint 4).
    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()>;

    /// The owner's recipe-created (non-favorite) products, grouped like
    /// `list_for_owner` (uncategorized last) and ordered by name within a
    /// group, each with the number of the owner's recipes that use it
    /// (MB-0014).
    async fn list_hidden_with_usage(&self, owner_id: Uuid) -> CoreResult<Vec<HiddenProductGroup>>;

    /// Make a recipe-created product a favorite: the same row, keeping its
    /// name and category, appended at the end of that category. A no-op for
    /// a favorite. `Invalid` while it has no category (RR-0001: a favorite
    /// needs one); `NotFound` if it isn't the owner's.
    async fn make_favorite(&self, owner_id: Uuid, id: Uuid) -> CoreResult<Product>;
}

/// Named shopping lists (MB-0007), shared with other users (MB-0008).
///
/// Every method that takes a list id authorizes `user_id` against that list
/// through one central access predicate (RR-0002 constraint 1): the owner, or
/// a `list_shares` grantee whose role is at least what the method needs.
/// A user with no access gets `NotFound`, so a list's existence never leaks;
/// a grantee whose role is too weak gets `Forbidden`. Access is read from the
/// database on every call, so a revoked grant stops working at once
/// (constraint 4).
///
/// Required access per method: reads (`get`, `access`, `items`,
/// `store_view`) need any grant (`view`); item writes (`add_item`,
/// `set_checked`, `remove_item`, `restore_item`) need `edit`; the methods
/// that read the owner's catalog (`add_product`, `add_recipe`,
/// `assign_category`) and administration (`rename`, `delete`, `shares`,
/// `share`, `revoke`) are the owner's alone.
#[async_trait]
pub trait ShoppingListRepo: Send + Sync {
    /// The user's own lists, most recently updated first. Lists shared with
    /// them are in [`ShoppingListRepo::shared_with`].
    async fn list_for_owner(&self, user_id: Uuid) -> CoreResult<Vec<ShoppingList>>;

    /// Lists other users shared with `user_id`, most recently updated first.
    async fn shared_with(&self, user_id: Uuid) -> CoreResult<Vec<SharedList>>;

    /// Create an empty list owned by `user_id`. Rejects an empty name
    /// (`Invalid`); names need not be unique.
    async fn create(&self, user_id: Uuid, name: &str) -> CoreResult<ShoppingList>;

    /// A list the user can see, to open and continue.
    async fn get(&self, user_id: Uuid, id: Uuid) -> CoreResult<ShoppingList>;

    /// What the user may do with a list (`NotFound` if nothing).
    async fn access(&self, user_id: Uuid, id: Uuid) -> CoreResult<ListAccess>;

    /// Rename a list (owner only); it counts as an update, so it moves to the
    /// top of the index. Rejects an empty name (`Invalid`).
    async fn rename(&self, user_id: Uuid, id: Uuid, name: &str) -> CoreResult<ShoppingList>;

    /// Delete a list (owner only); its items and shares go with it
    /// (`ON DELETE CASCADE`).
    async fn delete(&self, user_id: Uuid, id: Uuid) -> CoreResult<()>;

    /// A list's items in position order: the active ones by default
    /// (`removed_at IS NULL`, RR-0001 constraint 7), or only the removed ones.
    async fn items(&self, user_id: Uuid, list_id: Uuid, removed: bool)
    -> CoreResult<Vec<ListItem>>;

    /// Append a free-text item (`edit`). It is uncategorized until a
    /// category is assigned (RR-0001 constraint 5). Rejects empty text
    /// (`Invalid`).
    async fn add_item(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        text: &str,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem>;

    /// Append an item for one of the owner's products, copying its name and
    /// category now (RR-0001 constraint 6). Owner only. `Invalid` for a
    /// product that isn't the owner's.
    async fn add_product(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        product_id: Uuid,
        quantity: Option<&str>,
    ) -> CoreResult<ListItem>;

    /// Append one item per ingredient of one of the owner's recipes, in
    /// recipe order and atomically (MB-0005). Owner only. Each item copies,
    /// at add-time, the ingredient's product, that product's current name
    /// and category, and the ingredient's quantity with its leading number
    /// scaled by `multiplier` (`domain::quantity::scale`). An ingredient
    /// whose product is gone or uncategorized becomes an uncategorized item.
    /// The recipe and the catalog are only read. `NotFound` for a list or
    /// recipe that isn't the user's (`Forbidden` for a list only shared
    /// with them); `Invalid` for a recipe without ingredients or a
    /// multiplier outside (0, 100].
    async fn add_recipe(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        recipe_id: Uuid,
        multiplier: f64,
    ) -> CoreResult<Vec<ListItem>>;

    /// Put an item in one of the owner's categories. Owner only. Changes
    /// only this item, never the catalog. `Invalid` for a category that
    /// isn't the owner's.
    async fn assign_category(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        item_id: Uuid,
        category_id: Uuid,
    ) -> CoreResult<ListItem>;

    /// A list's active items grouped for the store (MB-0006). Groups follow
    /// the item's own (snapshotted) category, in the list owner's category
    /// order with name as tiebreak; uncategorized items form one last group.
    /// Within a group, items whose product is still in that category follow
    /// the product's position there, and the rest follow by name, ignoring
    /// case; position on the list breaks remaining ties. Empty groups are
    /// omitted.
    async fn store_view(&self, user_id: Uuid, list_id: Uuid) -> CoreResult<Vec<ListGroup>>;

    /// Mark an active item found (`checked`) or not (`edit`). Setting the
    /// state rather than flipping it makes a repeated request harmless.
    async fn set_checked(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        item_id: Uuid,
        checked: bool,
    ) -> CoreResult<ListItem>;

    /// Soft-delete an active item (sets `removed_at`; `edit`).
    async fn remove_item(&self, user_id: Uuid, list_id: Uuid, item_id: Uuid) -> CoreResult<()>;

    /// Put a removed item back on the list, in its old position (`edit`).
    async fn restore_item(&self, user_id: Uuid, list_id: Uuid, item_id: Uuid) -> CoreResult<()>;

    /// The list's grants, oldest first (owner only).
    async fn shares(&self, user_id: Uuid, list_id: Uuid) -> CoreResult<Vec<ListShare>>;

    /// Grant the account whose email matches `email` (ignoring ASCII case)
    /// `role` on the list, or change an existing grant's role (owner only).
    /// Returns the grant, or `None` when no single account other than the
    /// owner matches: the caller must answer both the same way, so the
    /// result never tells the owner whether an account exists (RR-0002
    /// constraint 3). `Invalid` for an empty email.
    async fn share(
        &self,
        user_id: Uuid,
        list_id: Uuid,
        email: &str,
        role: ShareRole,
    ) -> CoreResult<Option<ListShare>>;

    /// Delete a grant (owner only); the grantee loses access on their next
    /// request. Returns the revoked grant. `NotFound` for a grant that isn't
    /// on this list.
    async fn revoke(&self, user_id: Uuid, list_id: Uuid, share_id: Uuid) -> CoreResult<ListShare>;
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

    /// Delete a recipe and, by cascade, its ingredients. The products those
    /// ingredients used stay in the catalog unchanged (RR-0001 constraint 4).
    async fn delete(&self, owner_id: Uuid, id: Uuid) -> CoreResult<()>;

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
