//! SQLite-backed persistence.
//!
//! Uses runtime-checked `sqlx::query` so the scaffold builds without
//! `DATABASE_URL`. Once the schema is stable, switch to the `query!` macros +
//! `cargo sqlx prepare` offline cache for compile-time checking — tracked in
//! `.agentic/mentorship/persistence.md`.
//!
//! One file per repo trait implementation (RR-0009): `user`, `category`,
//! `product`, `list` and `recipe`; `invitation` holds the invitation and
//! admin repos (MB-0023). Row parsers and checks used by more than
//! one live here.

use chrono::{DateTime, SubsecRound, Utc};
use sqlx::Row;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteRow, SqliteSynchronous,
};
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{Category, Ingredient, Product};
use crate::error::CoreError;

mod category;
mod invitation;
mod list;
mod list_rows;
mod product;
mod recipe;
mod user;

#[cfg(test)]
mod tests;

/// Pool size. SQLite serializes writers, so more connections only add
/// readers (persistence.md: don't raise without benchmarking contention).
const MAX_CONNECTIONS: u32 = 8;

/// How long a connection waits for another's write lock before failing.
/// Writes are a few milliseconds, so even all other connections writing
/// ahead clears well inside this; it stays below the pool's 30 s acquire
/// timeout so a stuck lock surfaces as an error, not a hung request.
const BUSY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug)]
pub struct SqlitePool {
    inner: sqlx::SqlitePool,
}

impl SqlitePool {
    /// Open the pool. Every connection gets the same settings (MB-0025):
    ///
    /// - WAL journal, so readers don't block the writer or each other. WAL
    ///   is persistent in the file: the first connection switches an
    ///   existing rollback-journal database over, the rest find it set.
    ///   The database is then three files (`.db`, `-wal`, `-shm`); copying
    ///   the `.db` alone is not a backup.
    /// - `synchronous=NORMAL`: in WAL mode the database can't be corrupted,
    ///   and an app crash loses nothing; a power or OS failure can roll back
    ///   the last commits since the WAL was last synced.
    /// - A busy timeout, so a write that finds another write in progress
    ///   waits instead of failing with "database is locked". Writes take the
    ///   lock up front (`BEGIN IMMEDIATE`), so the wait always applies.
    /// - Foreign keys on.
    ///
    /// `:memory:` databases (the tests) report journal mode `memory`.
    pub async fn connect(url: &str) -> CoreResult<Self> {
        let options = SqliteConnectOptions::from_str(url)
            .map_err(|e| CoreError::Invalid(format!("bad database url: {e}")))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(BUSY_TIMEOUT)
            .foreign_keys(true);

        let inner = SqlitePoolOptions::new()
            .max_connections(MAX_CONNECTIONS)
            .connect_with(options)
            .await?;

        Ok(Self { inner })
    }

    pub async fn run_migrations(&self) -> CoreResult<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.inner)
            .await
            .map_err(|e| CoreError::Invalid(format!("migration failed: {e}")))?;
        Ok(())
    }
}

/// `t` without its fraction of a second. Timestamps that SQL compares as
/// text (`expires_at > ?`) are bound at whole seconds, so sqlx's RFC3339 text
/// has a fixed width (RR-0012 constraint 7).
fn whole_seconds(t: DateTime<Utc>) -> DateTime<Utc> {
    t.trunc_subsecs(0)
}

fn parse_uuid(s: &str) -> CoreResult<Uuid> {
    Uuid::parse_str(s).map_err(|e| CoreError::Invalid(format!("invalid uuid: {e}")))
}

fn row_to_category(row: &SqliteRow) -> CoreResult<Category> {
    let id: String = row.try_get("id")?;
    let owner_id: String = row.try_get("owner_id")?;
    Ok(Category {
        id: parse_uuid(&id)?,
        owner_id: parse_uuid(&owner_id)?,
        name: row.try_get("name")?,
        position: row.try_get("position")?,
        created_at: row.try_get("created_at")?,
    })
}

fn row_to_product(row: &SqliteRow) -> CoreResult<Product> {
    let id: String = row.try_get("id")?;
    let owner_id: String = row.try_get("owner_id")?;
    let category_id: Option<String> = row.try_get("category_id")?;
    Ok(Product {
        id: parse_uuid(&id)?,
        owner_id: parse_uuid(&owner_id)?,
        name: row.try_get("name")?,
        category_id: category_id.as_deref().map(parse_uuid).transpose()?,
        position: row.try_get("position")?,
        is_favorite: row.try_get("is_favorite")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

/// Trim a user-supplied name and reject it if empty. `what` names the entity
/// in the error message.
fn clean_name<'a>(what: &str, name: &'a str) -> CoreResult<&'a str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CoreError::Invalid(format!("{what} name must not be empty")));
    }
    Ok(trimmed)
}

/// Map a unique-constraint violation to `Conflict`; pass other errors through.
fn conflict_on_unique(e: sqlx::Error, message: impl FnOnce() -> String) -> CoreError {
    match &e {
        sqlx::Error::Database(db) if db.is_unique_violation() => CoreError::Conflict(message()),
        _ => CoreError::Database(e),
    }
}

fn unknown_category() -> CoreError {
    CoreError::Invalid("unknown category".to_owned())
}

// Shared by the recipe and list repos (`add_recipe` copies ingredients).

/// Map a row of [`INGREDIENT_SELECT`] to an ingredient.
fn row_to_ingredient(row: &SqliteRow, owner_id: Uuid) -> CoreResult<Ingredient> {
    let id: String = row.try_get("id")?;
    let recipe_id: String = row.try_get("recipe_id")?;
    let product_id: Option<String> = row.try_get("product_id")?;
    let category_id: Option<String> = row.try_get("category_id")?;
    let category = category_id
        .as_deref()
        .map(|id| -> CoreResult<Category> {
            Ok(Category {
                id: parse_uuid(id)?,
                owner_id,
                name: row.try_get("category_name")?,
                position: row.try_get("category_position")?,
                created_at: row.try_get("category_created_at")?,
            })
        })
        .transpose()?;
    Ok(Ingredient {
        id: parse_uuid(&id)?,
        recipe_id: parse_uuid(&recipe_id)?,
        product_id: product_id.as_deref().map(parse_uuid).transpose()?,
        text: row.try_get("text")?,
        quantity: row.try_get("quantity")?,
        position: row.try_get("position")?,
        category,
    })
}

/// A recipe's ingredients with their products' live names and categories.
/// Binds: `?1` recipe id, `?2` owner id, `?3` one ingredient id or NULL for
/// all. Every join checks the owner, so another user's product or category
/// can never show up here (RR-0001 constraint 9).
const INGREDIENT_SELECT: &str = r"
    SELECT i.id, i.recipe_id, p.id AS product_id, COALESCE(p.name, i.text) AS text,
           i.quantity, i.position,
           c.id AS category_id, c.name AS category_name,
           c.position AS category_position, c.created_at AS category_created_at
      FROM ingredients i
      JOIN recipes r ON r.id = i.recipe_id AND r.owner_id = ?2
      LEFT JOIN products p ON p.id = i.product_id AND p.owner_id = r.owner_id
      LEFT JOIN categories c ON c.id = p.category_id AND c.owner_id = r.owner_id
     WHERE i.recipe_id = ?1 AND (?3 IS NULL OR i.id = ?3)
     ORDER BY i.position ASC, i.id ASC";

/// Trim a quantity; blank means "no quantity".
fn clean_quantity(quantity: Option<&str>) -> Option<&str> {
    quantity.map(str::trim).filter(|q| !q.is_empty())
}
