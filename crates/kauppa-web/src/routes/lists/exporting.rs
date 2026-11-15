//! Export a list's items to the user's favorites (MB-0060, RR-0047).
//!
//! Anyone who can open the list exports into their own catalog. The list is
//! read through `ShoppingListRepo` (the access predicate, at `view`), and
//! `ProductRepo::apply_export` plans and writes in one transaction on the
//! user's products. The review and summary pages are `routes::review`'s,
//! shared with favorites import (MB-0061).
//!
//! - `POST /lists/{id}/export` saves at once when nothing needs an answer,
//!   else shows the review page.
//! - `POST /lists/{id}/export/confirm` carries the review's answers and its
//!   fingerprint. A changed list or catalog, or a missing answer, shows the
//!   review again and saves nothing; a tampered answer is a 400.
//! - Saving answers 303 to `GET /lists/{id}/export/done`, the summary.

use actix_identity::Identity;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use uuid::Uuid;

use kauppa_core::domain::ShoppingList;
use kauppa_core::export::{Candidate, ExportOutcome};
use kauppa_core::repo::{ProductRepo, ShoppingListRepo};

use crate::error::WebResult;
use crate::i18n::Ui;
use crate::routes::request::current_user;
use crate::routes::review::{self, DoneQuery, Page, Source};
use crate::state::AppState;

fn page(list: &ShoppingList) -> Page {
    let base = format!("/lists/{}/export", list.id);
    Page {
        action: format!("{base}/confirm"),
        base,
        back: format!("/lists/{}", list.id),
        source: Source::List(list.name.clone()),
        hidden: Vec::new(),
    }
}

/// The list's active items, read through the access predicate.
async fn candidates(state: &AppState, user: Uuid, list: Uuid) -> WebResult<Vec<Candidate>> {
    Ok(ShoppingListRepo::items(&state.pool, user, list, false)
        .await?
        .iter()
        .map(Candidate::from)
        .collect())
}

fn log_saved(user: Uuid, list: Uuid, outcome: &ExportOutcome) {
    if let ExportOutcome::Saved(s) = outcome {
        tracing::info!(user_id = %user, list_id = %list, added = s.added, "list exported to favorites");
    }
}

/// Start an export: save, or show the review.
#[post("/{id}/export")]
pub async fn export(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
    let items = candidates(&state, user, list_id).await?;
    let outcome = ProductRepo::apply_export(&state.pool, user, &items, None).await?;
    log_saved(user, list_id, &outcome);
    review::respond(&state, &ui, &req, &page(&list), outcome)
}

/// Confirm a review: the answers and the fingerprint they were made against.
#[post("/{id}/export/confirm")]
pub async fn export_confirm(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<Vec<(String, String)>>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let (answers, _) = review::answers(form.into_inner());
    let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
    let items = candidates(&state, user, list_id).await?;
    let outcome = ProductRepo::apply_export(&state.pool, user, &items, Some(&answers)).await?;
    log_saved(user, list_id, &outcome);
    review::respond(&state, &ui, &req, &page(&list), outcome)
}

/// The summary after a save.
#[get("/{id}/export/done")]
pub async fn export_done(
    state: web::Data<AppState>,
    ui: Ui,
    id: Identity,
    path: web::Path<Uuid>,
    query: web::Query<DoneQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list = ShoppingListRepo::get(&state.pool, user, path.into_inner()).await?;
    let page = page(&list);
    review::done(&state, &ui, page.back, &page.source, query.into_inner())
}
