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
//! requests with a 303 back to the list. One add field takes both (MB-0036):
//! the owner's text that names one of their favorites (`name_key`, RR-0003)
//! adds that favorite, whose item copies its name and category at add-time
//! (RR-0001), and the field suggests the owner's favorites from a
//! `<datalist>` outside the fragment; any other text is a free-text item that
//! stays uncategorized until the owner picks a category. Adding a recipe (MB-0005) adds one item per
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
//! grantee can add free-text items (their text never matches a favorite, and
//! they get no suggestions: MB-0036, CRP-0024), change quantities (MB-0021), check,
//! remove and restore; favorites, recipes and categories come from the
//! owner's catalog and stay the owner's, as do rename, delete and
//! `/lists/{id}/sharing`. Sharing answers
//! the same whether or not the email matches an account (RR-0002
//! constraint 3), and grants and revokes are logged without the grantee's
//! email (constraint 5).
//!
//! Lists can be archived by their owner (MB-0062, RR-0045). An archived list
//! leaves the index's main list and "Shared with you" for a collapsed
//! "Archived" section (a `<details>`, open only after an action inside it),
//! most recently archived first. Its page and store view are read-only, with
//! a notice and, for the owner, Restore: the repo refuses item and recipe
//! changes (`Conflict::Archived`, the 409 page), and their controls aren't
//! rendered. Rename, delete and sharing stay the owner's as before.
//!
//! Split by area (MB-0019, RR-0009): this file has the shared views and
//! rendering, and the list index and lifecycle handlers; `items` (MB-0004,
//! MB-0005), `store` (MB-0006), `shares` (MB-0008) and `archiving` (MB-0062)
//! have the rest. Tests are in `tests/<area>.rs`.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use kauppa_core::Invalid;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{
    ArchivedList, Category, ListAccess, ListItem, ProductGroup, ShareRole, SharedList, ShoppingList,
};
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::i18n::{Ui, VARY};
use crate::routes::products::Picker;
use crate::routes::request::{current_user, is_hx, wants_fragment};
use crate::state::AppState;

mod archiving;
mod items;
mod shares;
mod store;
#[cfg(test)]
mod tests;

pub use archiving::{archive, unarchive};
pub use items::{add_item, add_recipe, remove_item, restore_item, update_item};
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
    /// Archived lists, own and shared, most recently archived first
    /// (MB-0062).
    archived: Vec<ArchivedCard>,
    archived_count: usize,
    /// Render the Archived section open: the action happened inside it.
    archived_open: bool,
    draft_name: String,
    error: Option<String>,
}

/// `GET /lists?open=archived`: the Archived section open, as the redirect
/// after a plain action inside it names it (MB-0062 P5).
#[derive(Debug, Deserialize)]
struct IndexQuery {
    open: Option<String>,
}

/// The `?open=` value that opens the Archived section.
const OPEN_ARCHIVED: &str = "archived";

/// An archived list on the index (MB-0062): the owner's controls (Restore,
/// rename, delete) or, for a grantee, whose list it is.
#[derive(Debug, Serialize)]
struct ArchivedCard {
    id: Uuid,
    name: String,
    /// When it was archived, as a day.
    archived: String,
    is_owner: bool,
    owner_name: String,
}

impl ArchivedCard {
    fn new(archived: ArchivedList, state: &AppState, ui: &Ui) -> WebResult<Self> {
        let at = archived
            .list
            .archived_at
            .unwrap_or(archived.list.updated_at);
        Ok(Self {
            id: archived.list.id,
            name: archived.list.name,
            archived: ui.date(state, at)?,
            is_owner: archived.access == ListAccess::Owner,
            owner_name: archived.owner_name,
        })
    }
}

/// The notice on an archived list's page and store view (RR-0045): when it
/// was archived, and Restore for the owner.
#[derive(Debug, Serialize)]
struct ArchivedNotice {
    date: String,
    can_restore: bool,
}

impl ArchivedNotice {
    fn new(
        list: &ShoppingList,
        access: ListAccess,
        state: &AppState,
        ui: &Ui,
    ) -> WebResult<Option<Self>> {
        list.archived_at
            .map(|at| {
                Ok(Self {
                    date: ui.date(state, at)?,
                    can_restore: access == ListAccess::Owner,
                })
            })
            .transpose()
    }
}

#[derive(Debug, Serialize)]
struct SharedCard {
    id: Uuid,
    name: String,
    owner_name: String,
    can_edit: bool,
    updated: String,
}

impl SharedCard {
    fn new(shared: SharedList, state: &AppState, ui: &Ui) -> WebResult<Self> {
        let card = ListCard::new(shared.list, state, ui)?;
        Ok(Self {
            id: card.id,
            name: card.name,
            owner_name: shared.owner_name,
            can_edit: shared.role == ShareRole::Edit,
            updated: card.updated,
        })
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
    /// The owner's favorite names for the add field's `<datalist>`
    /// (MB-0036). On the page, outside `#list-{id}`, so fragments don't
    /// resend them; empty for grantees.
    suggestions: Vec<String>,
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
    /// The owner's favorites, which the add field suggests (MB-0036); none
    /// shows a hint.
    favorite_count: usize,
    recipes: Vec<RecipeOption>,
    has_categories: bool,
    /// The owner: the sharing link and, on an active list, the catalog
    /// controls (favorites hint, add a recipe).
    is_owner: bool,
    /// The owner or an `edit` grantee, on an active list: item controls.
    can_edit: bool,
    /// Set when the list is archived (MB-0062): read-only, with a notice.
    archived: Option<ArchivedNotice>,
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
        picker: Picker::optional(categories, category_id),
    }
}

/// The favorites' names, in the favorites page's order (by category).
fn favorite_names(groups: Vec<ProductGroup>) -> Vec<String> {
    groups
        .into_iter()
        .flat_map(|g| g.products)
        .map(|p| p.name)
        .collect()
}

/// Render an open list as the full page or, for htmx, its `#list-{id}`
/// fragment.
async fn render_list(
    ui: &Ui,
    user: Uuid,
    list_id: Uuid,
    fragment: bool,
    status: StatusCode,
    draft: ItemDraft,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let state = ui.state();
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
    let archived = ArchivedNotice::new(&list, access, state, ui)?;
    let active = archived.is_none();
    let card = ListCard::new(list, state, ui)?;
    let removed_count = removed.len();
    let suggestions = favorite_names(favorites);
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
        favorite_count: suggestions.len(),
        recipes: recipes
            .into_iter()
            .map(|r| RecipeOption {
                id: r.id,
                title: r.title,
            })
            .collect(),
        has_categories: !categories.is_empty(),
        is_owner,
        can_edit: access >= ListAccess::Edit && active,
        archived,
        draft,
        error,
    };
    let body = if fragment {
        ui.render(state, "lists/_list", &view)?
    } else {
        ui.render(
            state,
            "lists/show",
            &ShowView {
                list: view,
                suggestions,
            },
        )?
    };
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .insert_header(VARY)
        .body(body))
}

/// Answer an item mutation on an open list, like `respond` does for the
/// index: the fragment for htmx (rejections included, with 200), else a 303
/// back to the list, or the full page with 400 for rejected input.
async fn respond_item<T>(
    ui: &Ui,
    req: &HttpRequest,
    user: Uuid,
    list_id: Uuid,
    outcome: CoreResult<T>,
    draft: ItemDraft,
) -> WebResult<HttpResponse> {
    let error = match outcome {
        Ok(_) => None,
        Err(CoreError::Invalid(m)) => {
            tracing::debug!(user_id = %user, list_id = %list_id, reason = %m, "list item rejected");
            Some(ui.invalid(&m)?)
        }
        Err(e) => return Err(e.into()),
    };

    if is_hx(req) {
        render_list(ui, user, list_id, true, StatusCode::OK, draft, error).await
    } else if error.is_none() {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, format!("/lists/{list_id}")))
            .finish())
    } else {
        let status = StatusCode::BAD_REQUEST;
        render_list(ui, user, list_id, false, status, draft, error).await
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
    raw.replace(',', ".")
        .parse::<f64>()
        .map_err(|_| CoreError::Invalid(Invalid::MultiplierNotNumber))
}

impl ListCard {
    /// The card for `list`, its update date in the request's language
    /// (MB-0035 P5).
    fn new(list: ShoppingList, state: &AppState, ui: &Ui) -> WebResult<Self> {
        Ok(Self {
            id: list.id,
            updated: ui.date(state, list.updated_at)?,
            name: list.name,
        })
    }
}

/// Render the index as the full page or, for htmx, the `#lists` fragment.
#[allow(clippy::too_many_arguments)]
async fn render_index(
    state: &AppState,
    ui: &Ui,
    user: Uuid,
    fragment: bool,
    status: StatusCode,
    archived_open: bool,
    draft_name: String,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let lists = ShoppingListRepo::list_for_owner(&state.pool, user).await?;
    let shared = ShoppingListRepo::shared_with(&state.pool, user).await?;
    let archived = ShoppingListRepo::archived(&state.pool, user).await?;
    let view = IndexView {
        lists: lists
            .into_iter()
            .map(|l| ListCard::new(l, state, ui))
            .collect::<WebResult<_>>()?,
        shared: shared
            .into_iter()
            .map(|s| SharedCard::new(s, state, ui))
            .collect::<WebResult<_>>()?,
        archived_count: archived.len(),
        archived: archived
            .into_iter()
            .map(|a| ArchivedCard::new(a, state, ui))
            .collect::<WebResult<_>>()?,
        archived_open,
        draft_name,
        error,
    };
    let template = if fragment {
        "lists/_index"
    } else {
        "lists/index"
    };
    let body = ui.render(state, template, &view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .insert_header(VARY)
        .body(body))
}

/// Answer an index mutation. Rejected input (`Invalid`) is shown to the user:
/// in the fragment for htmx — with 200, since htmx doesn't swap 4xx responses
/// by default — or on the full page with 400. Success is the fragment for
/// htmx, else a 303 to `location`. `NotFound` (another user's list) is a 404.
/// `in_archive`: the action happened inside the Archived section, which is
/// then rendered open (MB-0062 P5).
#[allow(clippy::too_many_arguments)]
async fn respond<T>(
    state: &AppState,
    ui: &Ui,
    req: &HttpRequest,
    user: Uuid,
    outcome: CoreResult<T>,
    location: impl FnOnce(&T) -> String,
    in_archive: bool,
    draft_name: String,
) -> WebResult<HttpResponse> {
    let outcome = match outcome {
        Ok(value) => Ok(value),
        Err(CoreError::Invalid(m)) => {
            tracing::debug!(user_id = %user, reason = %m, "list input rejected");
            Err(ui.invalid(&m)?)
        }
        Err(e) => return Err(e.into()),
    };

    match outcome {
        _ if is_hx(req) => {
            let error = outcome.err();
            let status = StatusCode::OK;
            render_index(state, ui, user, true, status, in_archive, draft_name, error).await
        }
        Ok(value) => Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, location(&value)))
            .finish()),
        Err(error) => {
            let status = StatusCode::BAD_REQUEST;
            let error = Some(error);
            render_index(
                state, ui, user, false, status, in_archive, draft_name, error,
            )
            .await
        }
    }
}

#[get("")]
pub async fn index(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    query: web::Query<IndexQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let archived_open = query.open.as_deref() == Some(OPEN_ARCHIVED);
    render_index(
        &state,
        &ui,
        user,
        false,
        StatusCode::OK,
        archived_open,
        String::new(),
        None,
    )
    .await
}

/// Whether `list` is archived, for answering an action on it inside the
/// Archived section (MB-0062 P5). Any error is the action's to report.
async fn is_archived(state: &AppState, user: Uuid, list: Uuid) -> bool {
    ShoppingListRepo::get(&state.pool, user, list)
        .await
        .is_ok_and(|l| l.archived_at.is_some())
}

/// The index, with the Archived section open when `in_archive`.
fn index_location(in_archive: bool) -> String {
    if in_archive {
        format!("/lists?open={OPEN_ARCHIVED}")
    } else {
        "/lists".to_owned()
    }
}

/// Create a list. Without htmx the user lands in the new list.
#[post("")]
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
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
        &ui,
        &req,
        user,
        outcome,
        |list| format!("/lists/{}", list.id),
        false,
        draft,
    )
    .await
}

/// Open a list to continue it.
#[get("/{id}")]
pub async fn show(
    req: HttpRequest,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let fragment = wants_fragment(&req);
    let draft = ItemDraft::default();
    render_list(
        &ui,
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
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<NameForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list = path.into_inner();
    let in_archive = is_archived(&state, user, list).await;
    let outcome = ShoppingListRepo::rename(&state.pool, user, list, &form.name).await;
    respond(
        &state,
        &ui,
        &req,
        user,
        outcome,
        |_| index_location(in_archive),
        in_archive,
        String::new(),
    )
    .await
}

#[post("/{id}/delete")]
pub async fn delete(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list = path.into_inner();
    let in_archive = is_archived(&state, user, list).await;
    let outcome = ShoppingListRepo::delete(&state.pool, user, list).await;
    respond(
        &state,
        &ui,
        &req,
        user,
        outcome,
        |()| index_location(in_archive),
        in_archive,
        String::new(),
    )
    .await
}
