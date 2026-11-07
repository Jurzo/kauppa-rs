//! Row parsing and grouping for the list repo (`sqlite/list.rs`). Plain Rust
//! with no SQL: every list query, and every mention of a list table, stays in
//! `sqlite/list.rs` (RR-0009 constraint 3).

use sqlx::Row;
use sqlx::sqlite::SqliteRow;
use uuid::Uuid;

use crate::CoreResult;
use crate::domain::{
    Category, ListAccess, ListGroup, ListItem, ListShare, ShareRole, ShoppingList,
};

use super::parse_uuid;

pub(super) fn row_to_list(row: &SqliteRow) -> CoreResult<ShoppingList> {
    let id: String = row.try_get("id")?;
    let owner_id: String = row.try_get("owner_id")?;
    Ok(ShoppingList {
        id: parse_uuid(&id)?,
        owner_id: parse_uuid(&owner_id)?,
        name: row.try_get("name")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

pub(super) fn parse_access(raw: &str) -> CoreResult<ListAccess> {
    if raw == "owner" {
        Ok(ListAccess::Owner)
    } else {
        ShareRole::parse(raw).map(ListAccess::from)
    }
}

/// Map a row of `ITEM_SELECT` (in `sqlite/list.rs`) to a list item.
pub(super) fn row_to_item(row: &SqliteRow, owner_id: Uuid) -> CoreResult<ListItem> {
    let id: String = row.try_get("id")?;
    let list_id: String = row.try_get("list_id")?;
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
    Ok(ListItem {
        id: parse_uuid(&id)?,
        list_id: parse_uuid(&list_id)?,
        product_id: product_id.as_deref().map(parse_uuid).transpose()?,
        text: row.try_get("text")?,
        quantity: row.try_get("quantity")?,
        checked: row.try_get("checked")?,
        position: row.try_get("position")?,
        category,
        removed_at: row.try_get("removed_at")?,
    })
}

pub(super) fn row_to_share(row: &SqliteRow) -> CoreResult<ListShare> {
    let id: String = row.try_get("id")?;
    let list_id: String = row.try_get("list_id")?;
    let grantee_id: String = row.try_get("grantee_id")?;
    let role: String = row.try_get("role")?;
    Ok(ListShare {
        id: parse_uuid(&id)?,
        list_id: parse_uuid(&list_id)?,
        grantee_id: parse_uuid(&grantee_id)?,
        grantee_name: row.try_get("grantee_name")?,
        grantee_email: row.try_get("grantee_email")?,
        role: ShareRole::parse(&role)?,
        created_at: row.try_get("created_at")?,
    })
}

/// Group store-view rows by category and order them (see
/// `ShoppingListRepo::store_view`).
pub(super) fn group_for_store(mut rows: Vec<(ListItem, Option<i64>)>) -> Vec<ListGroup> {
    // Category order: the owner's position, then name; uncategorized last.
    let group_key = |item: &ListItem| {
        item.category
            .as_ref()
            .map(|c| (c.position, c.name.to_lowercase(), c.id))
    };
    rows.sort_by(|(a, a_pos), (b, b_pos)| {
        let (ga, gb) = (group_key(a), group_key(b));
        ga.is_none()
            .cmp(&gb.is_none())
            .then_with(|| ga.cmp(&gb))
            // Within a group: product position first, then by name.
            .then_with(|| a_pos.is_none().cmp(&b_pos.is_none()))
            .then_with(|| a_pos.cmp(b_pos))
            .then_with(|| a.text.to_lowercase().cmp(&b.text.to_lowercase()))
            .then_with(|| a.position.cmp(&b.position))
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut groups: Vec<ListGroup> = Vec::new();
    for (item, _) in rows {
        match groups.last_mut() {
            Some(group)
                if group.category.as_ref().map(|c| c.id)
                    == item.category.as_ref().map(|c| c.id) =>
            {
                group.items.push(item);
            }
            _ => groups.push(ListGroup {
                category: item.category.clone(),
                items: vec![item],
            }),
        }
    }
    groups
}
