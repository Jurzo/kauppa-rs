//! Archive and restore a list (MB-0062, RR-0045): owner only, through the
//! repo's access predicate, so a grantee gets 403 and anyone else 404 as for
//! the list's other owner actions.
//!
//! From the index (htmx) both answer the `#lists` fragment; Restore, an
//! action inside the Archived section, renders it open. Without htmx, a 303
//! to `/lists` (`?open=archived` after Restore). The Restore on an archived
//! list's own page or store view sends `back`, and returns there.

use actix_identity::Identity;
use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, post, web};
use serde::Deserialize;
use uuid::Uuid;

use kauppa_core::repo::ShoppingListRepo;

use crate::error::WebResult;
use crate::i18n::Ui;
use crate::routes::request::current_user;
use crate::state::AppState;

use super::{index_location, respond};

/// Where Restore returns to when it isn't the index: the list's page
/// (`list`) or its store view (`shop`).
#[derive(Debug, Deserialize)]
struct RestoreForm {
    #[serde(default)]
    back: String,
}

/// Archive a list (owner only): it leaves the main list for the Archived
/// section, which stays closed.
#[post("/{id}/archive")]
pub async fn archive(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list = path.into_inner();
    let outcome = ShoppingListRepo::archive(&state.pool, user, list).await;
    if outcome.is_ok() {
        tracing::info!(user_id = %user, list_id = %list, "list archived");
    }
    respond(
        &state,
        &ui,
        &req,
        user,
        outcome,
        |_| index_location(false),
        false,
        String::new(),
    )
    .await
}

/// Restore an archived list (owner only): back to the main list in its usual
/// place.
#[post("/{id}/unarchive")]
pub async fn unarchive(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: Option<web::Form<RestoreForm>>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list = path.into_inner();
    let outcome = ShoppingListRepo::unarchive(&state.pool, user, list).await;
    if outcome.is_ok() {
        tracing::info!(user_id = %user, list_id = %list, "list restored from the archive");
    }
    // A post without a form body (no `back`) is a Restore from the index.
    let back = match form.as_ref().map_or("", |f| f.back.as_str()) {
        "list" => Some(format!("/lists/{list}")),
        "shop" => Some(format!("/lists/{list}/shop")),
        _ => None,
    };
    if let Some(back) = back {
        // From the list's own page (a plain or boosted form): back there.
        outcome?;
        return Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, back))
            .finish());
    }
    respond(
        &state,
        &ui,
        &req,
        user,
        outcome,
        |_| index_location(true),
        true,
        String::new(),
    )
    .await
}
