//! Named shopping lists — create, open to continue, rename, delete (MB-0007).
//!
//! Thin handlers: parse → call `ShoppingListRepo` → render. The index's
//! mutations answer htmx with the re-rendered `#lists` fragment; plain
//! requests get a 303 (PRG), or the full page when the input was rejected.
//! All endpoints require an authenticated user, and another user's list is a
//! 404.
//!
//! An open list is also where it is assembled (MB-0004): free-text items,
//! favorites, a category per item, and soft removal with restore. Those
//! mutations answer htmx with the re-rendered `#list-{id}` fragment, and plain
//! requests with a 303 back to the list. A favorite's item copies its name and
//! category at add-time (RR-0001); a free-text item stays uncategorized until
//! the user picks a category. Adding a recipe (MB-0005) adds one item per
//! ingredient the same way, with an optional quantity multiplier.
//!
//! `/lists/{id}/shop` is the store view (MB-0006): active items grouped by
//! their own category in the user's category order, with check-off and
//! removal. Its mutations answer htmx with the `#shop-{id}` fragment and
//! plain requests with a 303 back to it.
//!
//! Lists can be shared (MB-0008, RR-0002). Authorization is not decided here:
//! every `ShoppingListRepo` call runs the repo's central access predicate, so
//! a user with no access gets a 404 and a grantee whose role is too weak a
//! 403. Handlers only use the access level to decide which controls to show:
//! a `view` grantee sees the list and the store view read-only; an `edit`
//! grantee can add free-text items, check, remove and restore; favorites,
//! recipes and categories come from the owner's catalog and stay the
//! owner's, as do rename, delete and `/lists/{id}/sharing`. Sharing answers
//! the same whether or not the email matches an account (RR-0002
//! constraint 3), and grants and revokes are logged without the grantee's
//! email (constraint 5).
//!
//! Split by area (MB-0019, RR-0009): this file has the shared views and
//! rendering, and the list index and lifecycle handlers; `items` (MB-0004,
//! MB-0005), `store` (MB-0006) and `shares` (MB-0008) have the rest. Tests are
//! in `tests/<area>.rs`.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{
    Category, ListAccess, ListItem, ProductGroup, ShareRole, SharedList, ShoppingList,
};
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::routes::products::Picker;
use crate::routes::request::{current_user, is_hx, wants_fragment};
use crate::state::AppState;

mod items;
mod shares;
mod store;
#[cfg(test)]
mod tests;

pub use items::{add_favorite, add_item, add_recipe, assign_category, remove_item, restore_item};
pub use shares::{revoke, share, sharing};
pub use store::{check_item, remove_from_shop, shop};

#[derive(Debug, Deserialize)]
struct NameForm {
    name: String,
}

#[derive(Debug, Serialize)]
struct IndexView {
    lists: Vec<ListCard>,
    /// Lists other users shared with this one (MB-0008).
    shared: Vec<SharedCard>,
    draft_name: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct SharedCard {
    id: Uuid,
    name: String,
    owner_name: String,
    can_edit: bool,
    updated: String,
}

impl From<SharedList> for SharedCard {
    fn from(shared: SharedList) -> Self {
        let card = ListCard::from(shared.list);
        Self {
            id: card.id,
            name: card.name,
            owner_name: shared.owner_name,
            can_edit: shared.role == ShareRole::Edit,
            updated: card.updated,
        }
    }
}

#[derive(Debug, Serialize)]
struct ListCard {
    id: Uuid,
    name: String,
    /// Last update, as a day (`31.10.2026`).
    updated: String,
}

#[derive(Debug, Serialize)]
struct ShowView {
    list: ListView,
}

/// An open list and its builder (`lists/_list`).
#[derive(Debug, Serialize)]
struct ListView {
    id: Uuid,
    name: String,
    updated: String,
    items: Vec<ItemRow>,
    removed: Vec<ItemRow>,
    removed_count: usize,
    favorites: Vec<FavoriteGroup>,
    recipes: Vec<RecipeOption>,
    has_categories: bool,
    /// The owner: catalog controls and the sharing link.
    is_owner: bool,
    /// The owner or an `edit` grantee: item controls.
    can_edit: bool,
    draft: ItemDraft,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ItemRow {
    id: Uuid,
    text: String,
    quantity: String,
    category: Option<String>,
    picker: Picker,
}

/// Favorites for the "add a favorite" `<select>`, one `<optgroup>` per
/// category.
#[derive(Debug, Serialize)]
struct FavoriteGroup {
    name: Option<String>,
    products: Vec<FavoriteOption>,
}

#[derive(Debug, Serialize)]
struct FavoriteOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Serialize)]
struct RecipeOption {
    id: Uuid,
    title: String,
}

/// The add forms' values after a rejected submission.
#[derive(Debug, Default, Serialize)]
struct ItemDraft {
    text: String,
    quantity: String,
    /// The recipe multiplier as typed; the form shows 1 when empty.
    multiplier: String,
}

fn item_row(item: ListItem, categories: &[Category]) -> ItemRow {
    let category_id = item.category.as_ref().map(|c| c.id);
    ItemRow {
        id: item.id,
        text: item.text,
        quantity: item.quantity.unwrap_or_default(),
        category: item.category.map(|c| c.name),
        picker: Picker::new(categories, category_id),
    }
}

fn favorite_groups(groups: Vec<ProductGroup>) -> Vec<FavoriteGroup> {
    groups
        .into_iter()
        .map(|g| FavoriteGroup {
            name: g.category.map(|c| c.name),
            products: g
                .products
                .into_iter()
                .map(|p| FavoriteOption {
                    id: p.id,
                    name: p.name,
                })
                .collect(),
        })
        .collect()
}

/// Render an open list as the full page or, for htmx, its `#list-{id}`
/// fragment.
async fn render_list(
    state: &AppState,
    user: Uuid,
    list_id: Uuid,
    fragment: bool,
    status: StatusCode,
    draft: ItemDraft,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let access = ShoppingListRepo::access(&state.pool, user, list_id).await?;
    let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
    let items = ShoppingListRepo::items(&state.pool, user, list_id, false).await?;
    let removed = ShoppingListRepo::items(&state.pool, user, list_id, true).await?;
    // The catalog controls are the owner's; a grantee's own catalog has no
    // place on someone else's list, so it isn't even read.
    let is_owner = access == ListAccess::Owner;
    let (categories, favorites, recipes) = if is_owner {
        (
            CategoryRepo::list_for_owner(&state.pool, user).await?,
            ProductRepo::list_for_owner(&state.pool, user).await?,
            RecipeRepo::list_for_owner(&state.pool, user).await?,
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    let card = ListCard::from(list);
    let removed_count = removed.len();
    let view = ListView {
        id: card.id,
        name: card.name,
        updated: card.updated,
        items: items
            .into_iter()
            .map(|i| item_row(i, &categories))
            .collect(),
        removed: removed
            .into_iter()
            .map(|i| item_row(i, &categories))
            .collect(),
        removed_count,
        favorites: favorite_groups(favorites),
        recipes: recipes
            .into_iter()
            .map(|r| RecipeOption {
                id: r.id,
                title: r.title,
            })
            .collect(),
        has_categories: !categories.is_empty(),
        is_owner,
        can_edit: access >= ListAccess::Edit,
        draft,
        error,
    };
    let body = if fragment {
        state.templates.render("lists/_list", &view)?
    } else {
        state
            .templates
            .render("lists/show", &ShowView { list: view })?
    };
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// Answer an item mutation on an open list, like `respond` does for the
/// index: the fragment for htmx (rejections included, with 200), else a 303
/// back to the list, or the full page with 400 for rejected input.
async fn respond_item<T>(
    state: &AppState,
    req: &HttpRequest,
    user: Uuid,
    list_id: Uuid,
    outcome: CoreResult<T>,
    draft: ItemDraft,
) -> WebResult<HttpResponse> {
    let error = match outcome {
        Ok(_) => None,
        Err(CoreError::Invalid(m)) => Some(m),
        Err(e) => return Err(e.into()),
    };
    if let Some(reason) = &error {
        tracing::debug!(user_id = %user, list_id = %list_id, %reason, "list item rejected");
    }

    if is_hx(req) {
        render_list(state, user, list_id, true, StatusCode::OK, draft, error).await
    } else if error.is_none() {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, format!("/lists/{list_id}")))
            .finish())
    } else {
        let status = StatusCode::BAD_REQUEST;
        render_list(state, user, list_id, false, status, draft, error).await
    }
}

/// The recipe multiplier: blank means 1, and a decimal comma is accepted
/// ("1,5"). Its range is checked by the repo. Also used by the recipe page's
/// "Add to a list" form (MB-0015).
pub fn parse_multiplier(raw: &str) -> CoreResult<f64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(1.0);
    }
    raw.replace(',', ".").parse::<f64>().map_err(|_| {
        CoreError::Invalid("the multiplier must be a number, e.g. 2 or 1,5".to_owned())
    })
}

impl From<ShoppingList> for ListCard {
    fn from(list: ShoppingList) -> Self {
        Self {
            id: list.id,
            updated: list.updated_at.format("%-d.%-m.%Y").to_string(),
            name: list.name,
        }
    }
}

/// Render the index as the full page or, for htmx, the `#lists` fragment.
async fn render_index(
    state: &AppState,
    user: Uuid,
    fragment: bool,
    status: StatusCode,
    draft_name: String,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let lists = ShoppingListRepo::list_for_owner(&state.pool, user).await?;
    let shared = ShoppingListRepo::shared_with(&state.pool, user).await?;
    let view = IndexView {
        lists: lists.into_iter().map(ListCard::from).collect(),
        shared: shared.into_iter().map(SharedCard::from).collect(),
        draft_name,
        error,
    };
    let template = if fragment {
        "lists/_index"
    } else {
        "lists/index"
    };
    let body = state.templates.render(template, &view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// Answer an index mutation. Rejected input (`Invalid`) is shown to the user:
/// in the fragment for htmx — with 200, since htmx doesn't swap 4xx responses
/// by default — or on the full page with 400. Success is the fragment for
/// htmx, else a 303 to `location`. `NotFound` (another user's list) is a 404.
async fn respond<T>(
    state: &AppState,
    req: &HttpRequest,
    user: Uuid,
    outcome: CoreResult<T>,
    location: impl FnOnce(&T) -> String,
    draft_name: String,
) -> WebResult<HttpResponse> {
    let outcome = match outcome {
        Ok(value) => Ok(value),
        Err(CoreError::Invalid(m)) => Err(m),
        Err(e) => return Err(e.into()),
    };
    if let Err(reason) = &outcome {
        tracing::debug!(user_id = %user, %reason, "list input rejected");
    }

    match outcome {
        _ if is_hx(req) => {
            let error = outcome.err();
            render_index(state, user, true, StatusCode::OK, draft_name, error).await
        }
        Ok(value) => Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, location(&value)))
            .finish()),
        Err(error) => {
            let status = StatusCode::BAD_REQUEST;
            render_index(state, user, false, status, draft_name, Some(error)).await
        }
    }
}

#[get("")]
pub async fn index(state: web::Data<AppState>, id: Identity) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    render_index(&state, user, false, StatusCode::OK, String::new(), None).await
}

/// Create a list. Without htmx the user lands in the new list.
#[post("")]
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    form: web::Form<NameForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ShoppingListRepo::create(&state.pool, user, &form.name).await;
    let draft = if outcome.is_ok() {
        String::new()
    } else {
        form.name.clone()
    };
    respond(
        &state,
        &req,
        user,
        outcome,
        |list| format!("/lists/{}", list.id),
        draft,
    )
    .await
}

/// Open a list to continue it.
#[get("/{id}")]
pub async fn show(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let fragment = wants_fragment(&req);
    let draft = ItemDraft::default();
    render_list(
        &state,
        user,
        path.into_inner(),
        fragment,
        StatusCode::OK,
        draft,
        None,
    )
    .await
}

#[post("/{id}/rename")]
pub async fn rename(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<NameForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ShoppingListRepo::rename(&state.pool, user, path.into_inner(), &form.name).await;
    respond(
        &state,
        &req,
        user,
        outcome,
        |_| "/lists".to_owned(),
        String::new(),
    )
    .await
}

#[post("/{id}/delete")]
pub async fn delete(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ShoppingListRepo::delete(&state.pool, user, path.into_inner()).await;
    respond(
        &state,
        &req,
        user,
        outcome,
        |()| "/lists".to_owned(),
        String::new(),
    )
    .await
}
