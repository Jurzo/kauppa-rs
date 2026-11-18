//! Request helpers shared by the route modules (MB-0019; they were
//! duplicated in each).

use actix_identity::Identity;
use actix_web::HttpRequest;
use uuid::Uuid;

use kauppa_core::CoreError;

use crate::error::{WebError, WebResult};

/// The signed-in user's id; `Unauthorized` without a valid identity.
pub fn current_user(id: &Identity) -> WebResult<Uuid> {
    let raw = id
        .id()
        .map_err(|_| WebError::Core(CoreError::Unauthorized))?;
    Uuid::parse_str(&raw).map_err(|_| WebError::Core(CoreError::Unauthorized))
}

/// Whether htmx sent the request (`HX-Request: true`), boosted or not.
pub fn is_hx(req: &HttpRequest) -> bool {
    req.headers().get("HX-Request").is_some_and(|v| v == "true")
}

/// An htmx request for part of a page. Boosted navigation (`hx-boost` on
/// `<body>`) also sends `HX-Request`, but swaps the whole body, so it gets the
/// full page.
pub fn wants_fragment(req: &HttpRequest) -> bool {
    is_hx(req) && req.headers().get("HX-Boosted").is_none()
}

/// The `id` field of a drag-to-reorder form (MB-0066, RR-0053 sub-answer
/// 4): the rows' ids, in their new order.
pub const ORDER_FIELD: &str = "id";

/// A full new order from a form's repeated [`ORDER_FIELD`]s, or `None` if
/// any value isn't an id; the repos then check it is exactly the current
/// set, so a stale or tampered order saves nothing.
#[must_use]
pub fn order_ids(form: &[(String, String)]) -> Option<Vec<Uuid>> {
    form.iter()
        .filter(|(name, _)| name == ORDER_FIELD)
        .map(|(_, value)| Uuid::parse_str(value.trim()).ok())
        .collect()
}
