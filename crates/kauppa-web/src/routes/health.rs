//! The health check (MB-0046, RR-0029 sub-answer 4): `200 ok` when the
//! database answers a trivial query, `503 unavailable` when it doesn't. No
//! detail either way: anyone can call it.

use actix_web::{HttpResponse, get, web};

use crate::state::AppState;

#[get("/healthz")]
pub async fn health(state: web::Data<AppState>) -> HttpResponse {
    match state.pool.ping().await {
        Ok(()) => HttpResponse::Ok().content_type("text/plain").body("ok"),
        Err(e) => {
            tracing::warn!(error = ?e, "health check: database unavailable");
            HttpResponse::ServiceUnavailable()
                .content_type("text/plain")
                .body("unavailable")
        }
    }
}

#[cfg(test)]
mod tests;
