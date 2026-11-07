//! The store view (MB-0006): `/lists/{id}/shop`, grouped by category, with check-off and remove.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{ListAccess, ListGroup};
use kauppa_core::repo::ShoppingListRepo;
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::routes::request::{current_user, is_hx, wants_fragment};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct CheckForm {
    #[serde(default)]
    checked: String,
}

/// The store view (`lists/shop`, fragment `lists/_shop`).
#[derive(Debug, Serialize)]
struct ShopView {
    id: Uuid,
    name: String,
    groups: Vec<ShopGroup>,
    found: usize,
    total: usize,
    /// The owner or an `edit` grantee: check-off and removal.
    can_edit: bool,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ShopGroup {
    /// `None` for the trailing uncategorized group.
    name: Option<String>,
    items: Vec<ShopItem>,
}

#[derive(Debug, Serialize)]
struct ShopItem {
    id: Uuid,
    text: String,
    quantity: String,
    checked: bool,
}

/// Render the store view as the full page or, for htmx, the `#shop-{id}`
/// fragment.
async fn render_shop(
    state: &AppState,
    user: Uuid,
    list_id: Uuid,
    fragment: bool,
    status: StatusCode,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let access = ShoppingListRepo::access(&state.pool, user, list_id).await?;
    let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
    let groups = ShoppingListRepo::store_view(&state.pool, user, list_id).await?;
    let groups: Vec<ShopGroup> = groups
        .into_iter()
        .map(|ListGroup { category, items }| ShopGroup {
            name: category.map(|c| c.name),
            items: items
                .into_iter()
                .map(|i| ShopItem {
                    id: i.id,
                    text: i.text,
                    quantity: i.quantity.unwrap_or_default(),
                    checked: i.checked,
                })
                .collect(),
        })
        .collect();
    let items = || groups.iter().flat_map(|g| &g.items);
    let view = ShopView {
        id: list.id,
        name: list.name,
        found: items().filter(|i| i.checked).count(),
        total: items().count(),
        groups,
        can_edit: access >= ListAccess::Edit,
        error,
    };
    let template = if fragment {
        "lists/_shop"
    } else {
        "lists/shop"
    };
    let body = state.templates.render(template, &view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// Answer a store-view mutation: the fragment for htmx (a rejection
/// included, with 200), else a 303 back to the store view, or the full page
/// with 400 for rejected input. `NotFound` is a 404.
async fn respond_shop<T>(
    state: &AppState,
    req: &HttpRequest,
    user: Uuid,
    list_id: Uuid,
    outcome: CoreResult<T>,
) -> WebResult<HttpResponse> {
    let error = match outcome {
        Ok(_) => None,
        Err(CoreError::Invalid(m)) => Some(m),
        Err(e) => return Err(e.into()),
    };
    if is_hx(req) {
        render_shop(state, user, list_id, true, StatusCode::OK, error).await
    } else if error.is_none() {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, format!("/lists/{list_id}/shop")))
            .finish())
    } else {
        render_shop(state, user, list_id, false, StatusCode::BAD_REQUEST, error).await
    }
}

fn parse_checked(raw: &str) -> CoreResult<bool> {
    match raw.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(CoreError::Invalid("unknown found state".to_owned())),
    }
}

/// The store view: what to fetch, grouped by category.
#[get("/{id}/shop")]
pub async fn shop(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let fragment = wants_fragment(&req);
    render_shop(
        &state,
        user,
        path.into_inner(),
        fragment,
        StatusCode::OK,
        None,
    )
    .await
}

/// Mark an item found or not found. The form sends the state to set, so a
/// repeated tap (or a retry on a flaky connection) can't undo itself.
#[post("/{id}/shop/items/{item}/check")]
pub async fn check_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
    form: web::Form<CheckForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = match parse_checked(&form.checked) {
        Ok(checked) => {
            ShoppingListRepo::set_checked(&state.pool, user, list_id, item_id, checked).await
        }
        Err(e) => Err(e),
    };
    respond_shop(&state, &req, user, list_id, outcome).await
}

/// Remove an item from the store view (soft delete; it can be restored from
/// the list page).
#[post("/{id}/shop/items/{item}/remove")]
pub async fn remove_from_shop(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = ShoppingListRepo::remove_item(&state.pool, user, list_id, item_id).await;
    respond_shop(&state, &req, user, list_id, outcome).await
}
