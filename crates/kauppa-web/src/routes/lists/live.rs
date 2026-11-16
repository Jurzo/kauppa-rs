//! The list's event stream (MB-0057, RR-0049): `GET /lists/{id}/events`.
//!
//! In order: identity (401), the stream limits (503 with `Retry-After`,
//! before any query), then the central access predicate at `view` (404).
//! No stream opens before the predicate passes. The stream itself, and its
//! re-check before every event, are in [`crate::live`]; it carries no list
//! content.

use actix_identity::Identity;
use actix_web::http::header;
use actix_web::{HttpResponse, get, web};
use serde::Deserialize;
use uuid::Uuid;

use kauppa_core::repo::ShoppingListRepo;

use crate::error::WebResult;
use crate::live;
use crate::routes::request::current_user;
use crate::state::AppState;

/// Seconds a refused connect is told to wait (`Retry-After`).
const RETRY_AFTER_SECS: u64 = 30;

/// The stream URL's query. Neither value authorizes anything, and the
/// query is never logged (the request span records the path only).
#[derive(Debug, Deserialize)]
struct EventsQuery {
    /// The page's tab id (RR-0049 sub-answer 8).
    tab: Option<String>,
    /// Where the page's render started ([`live::LiveHub::mark`]).
    since: Option<String>,
}

#[get("/{id}/events")]
pub async fn events(
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    query: web::Query<EventsQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let slot = match state.live.reserve(user) {
        Ok(slot) => slot,
        Err(refused) => {
            tracing::warn!(user_id = %user, limit = refused.limit(), "live stream refused");
            return Ok(HttpResponse::ServiceUnavailable()
                .insert_header((header::RETRY_AFTER, RETRY_AFTER_SECS.to_string()))
                .insert_header((header::CACHE_CONTROL, "no-store"))
                .finish());
        }
    };
    // The predicate: `NotFound` (404) without access; any role may view.
    ShoppingListRepo::access(&state.pool, user, list_id).await?;
    let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
    let tab = query.tab.as_deref().and_then(|t| Uuid::parse_str(t).ok());
    let body = live::stream(
        slot,
        state.pool.clone(),
        list_id,
        list.owner_id,
        tab,
        query.since.as_deref(),
    );
    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .body(body))
}
