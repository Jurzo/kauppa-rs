//! The central list access predicate (RR-0002 constraint 1) and the helpers
//! that reach a list's rows only through a grant it made. With `list.rs` (the
//! `ShoppingListRepo` implementation), one of the two repository files that
//! name the list tables (RR-0009 constraint 3, as amended by RR-0036);
//! `repo::share`'s tests enforce it.

use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{ListAccess, ListItem, ShoppingList};
use crate::error::CoreError;

use super::SqlitePool;
use super::list_rows::{parse_access, row_to_item, row_to_list};

/// The central list access predicate (RR-0002 constraint 1, `security.md` →
/// Authorization): the lists `?1` (the acting user) owns, or holds a
/// `list_shares` grant on, each with the user's access — `owner`, or the
/// grant's role. `?2` narrows it to one list id; NULL means every list.
///
/// This is the only list query that decides access. Every
/// `ShoppingListRepo` method reaches list and item rows through it
/// (`accessible_lists` → `authorize` → `ListGrant`); `repo::share`'s tests
/// fail if a method or query goes around it.
const ACCESS_SELECT: &str = r"
    SELECT l.id, l.owner_id, l.name, l.created_at, l.updated_at,
           u.display_name AS owner_name,
           CASE WHEN l.owner_id = ?1 THEN 'owner' ELSE s.role END AS access
      FROM shopping_lists l
      JOIN users u ON u.id = l.owner_id
      LEFT JOIN list_shares s ON s.list_id = l.id AND s.grantee_id = ?1
     WHERE (l.owner_id = ?1 OR s.grantee_id IS NOT NULL)
       AND (?2 IS NULL OR l.id = ?2)
     ORDER BY l.updated_at DESC, l.id ASC";

/// One row of [`ACCESS_SELECT`].
pub(super) struct Accessible {
    pub(super) list: ShoppingList,
    pub(super) owner_name: String,
    pub(super) access: ListAccess,
}

/// Run the access predicate for `user`: every list they can reach, or only
/// `only`.
pub(super) async fn accessible_lists<'e, E>(
    exec: E,
    user: Uuid,
    only: Option<Uuid>,
) -> CoreResult<Vec<Accessible>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query(ACCESS_SELECT)
        .bind(user.to_string())
        .bind(only.map(|id| id.to_string()))
        .fetch_all(exec)
        .await?;
    rows.iter()
        .map(|row| {
            let access: String = row.try_get("access")?;
            Ok(Accessible {
                list: row_to_list(row)?,
                owner_name: row.try_get("owner_name")?,
                access: parse_access(&access)?,
            })
        })
        .collect()
}

/// Proof that the access predicate let a user act on `list` with `access`.
/// Only [`authorize`] makes one, and every query on a list's rows takes one,
/// so no list or item query can run before the predicate has. Its fields are
/// private to this file, so `list.rs` can read a grant but not build one
/// (RR-0036 sub-answer 3).
#[derive(Debug)]
pub(super) struct ListGrant {
    list: ShoppingList,
    access: ListAccess,
}

impl ListGrant {
    pub(super) fn list_id(&self) -> String {
        self.list.id.to_string()
    }

    /// The list owner: their catalog is the one the list's items join
    /// against, whoever is looking (RR-0001 constraint 9).
    pub(super) fn owner_id(&self) -> String {
        self.list.owner_id.to_string()
    }

    /// [`Self::owner_id`] as a `Uuid`.
    pub(super) const fn owner(&self) -> Uuid {
        self.list.owner_id
    }

    /// The access the predicate granted.
    pub(super) const fn access(&self) -> ListAccess {
        self.access
    }

    /// The granted list itself.
    pub(super) fn into_list(self) -> ShoppingList {
        self.list
    }
}

/// Decide whether `user` may act on `list_id` with at least `required`
/// access. `NotFound` when they have no access at all (the list's existence
/// stays hidden), `Forbidden` when their role is too weak.
async fn authorize<'e, E>(
    exec: E,
    user: Uuid,
    list_id: Uuid,
    required: ListAccess,
) -> CoreResult<ListGrant>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let found = accessible_lists(exec, user, Some(list_id))
        .await?
        .into_iter()
        .next()
        .ok_or(CoreError::NotFound)?;
    if found.access < required {
        return Err(CoreError::Forbidden);
    }
    Ok(ListGrant {
        list: found.list,
        access: found.access,
    })
}

/// A list's items with their categories' live names and order. Binds: `?1`
/// list id, `?2` the list owner's id (from a [`ListGrant`]), `?3` one item id
/// or NULL for all, `?4` 1 for removed items or 0 for active ones (ignored
/// when `?3` is set). Every join checks the owner (RR-0001 constraint 9).
const ITEM_SELECT: &str = r"
    SELECT i.id, i.list_id, p.id AS product_id, i.text, i.quantity, i.checked, i.position,
           i.removed_at,
           c.id AS category_id, c.name AS category_name,
           c.position AS category_position, c.created_at AS category_created_at
      FROM list_items i
      JOIN shopping_lists l ON l.id = i.list_id AND l.owner_id = ?2
      LEFT JOIN products p ON p.id = i.product_id AND p.owner_id = l.owner_id
      LEFT JOIN categories c ON c.id = i.category_id AND c.owner_id = l.owner_id
     WHERE i.list_id = ?1
       AND (CASE WHEN ?3 IS NULL THEN (i.removed_at IS NOT NULL) = ?4 ELSE i.id = ?3 END)
     ORDER BY i.position ASC, i.id ASC";

/// A list's items (see [`ITEM_SELECT`]), once the predicate has granted
/// access.
pub(super) async fn fetch_items<'e, E>(
    exec: E,
    grant: &ListGrant,
    only: Option<Uuid>,
    removed: bool,
) -> CoreResult<Vec<ListItem>>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let rows = sqlx::query(ITEM_SELECT)
        .bind(grant.list_id())
        .bind(grant.owner_id())
        .bind(only.map(|id| id.to_string()))
        .bind(removed)
        .fetch_all(exec)
        .await?;
    rows.iter()
        .map(|row| row_to_item(row, grant.list.owner_id))
        .collect()
}

pub(super) async fn fetch_item<'e, E>(exec: E, grant: &ListGrant, id: Uuid) -> CoreResult<ListItem>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    fetch_items(exec, grant, Some(id), false)
        .await?
        .into_iter()
        .next()
        .ok_or(CoreError::NotFound)
}

/// Mark a granted list updated, so a list being worked on rises to the top
/// of the index. Runs inside the caller's transaction so the item write and
/// `updated_at` commit together.
pub(super) async fn touch_list(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    grant: &ListGrant,
) -> CoreResult<()> {
    sqlx::query(r"UPDATE shopping_lists SET updated_at = ?1 WHERE id = ?2")
        .bind(Utc::now())
        .bind(grant.list_id())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

type Tx = sqlx::Transaction<'static, sqlx::Sqlite>;

impl SqlitePool {
    /// Open a read transaction and authorize `user` on `list_id` inside it,
    /// so the check and the reads that follow see one state.
    pub(super) async fn begin_read(
        &self,
        user: Uuid,
        list_id: Uuid,
        required: ListAccess,
    ) -> CoreResult<(Tx, ListGrant)> {
        let mut tx = self.inner.begin().await?;
        let grant = authorize(&mut *tx, user, list_id, required).await?;
        Ok((tx, grant))
    }

    /// Open a write transaction and authorize `user` on `list_id` inside it.
    /// `BEGIN IMMEDIATE` takes SQLite's write lock first, so a revoke can't
    /// commit between the check and the write (RR-0002 constraint 4), and a
    /// failure leaves nothing half-written.
    pub(super) async fn begin_write(
        &self,
        user: Uuid,
        list_id: Uuid,
        required: ListAccess,
    ) -> CoreResult<(Tx, ListGrant)> {
        let mut tx = self.inner.begin_with("BEGIN IMMEDIATE").await?;
        let grant = authorize(&mut *tx, user, list_id, required).await?;
        Ok((tx, grant))
    }
}
