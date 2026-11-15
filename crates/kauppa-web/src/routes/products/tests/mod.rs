//! Tests for `routes::products`, by area (RR-0009). Fixtures shared by several areas live here.

mod catalog;
mod edit;
mod hidden;

use uuid::Uuid;

use kauppa_core::repo::ProductRepo;

use crate::state::AppState;

/// Product names per group, in display order; `None` is uncategorized.
#[allow(clippy::expect_used)]
async fn catalog_shape(state: &AppState, user: Uuid) -> Vec<(Option<String>, Vec<String>)> {
    ProductRepo::list_for_owner(&state.pool, user)
        .await
        .expect("list products")
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name),
                g.products.into_iter().map(|p| p.name).collect(),
            )
        })
        .collect()
}

/// The catalog units (MB-0059) in `body`, in order, as `(dom id, open)`.
fn units(body: &str) -> Vec<(String, bool)> {
    body.match_indices("<details id=\"")
        .filter_map(|(at, start)| {
            let rest = &body[at + start.len()..];
            let id_end = rest.find('"')?;
            let tag = &rest[..rest.find('>')?];
            tag.contains("catalog-unit")
                .then(|| (rest[..id_end].to_owned(), tag.ends_with(" open")))
        })
        .collect()
}

/// The ids of the units `body` renders open.
fn open_units(body: &str) -> Vec<String> {
    units(body)
        .into_iter()
        .filter_map(|(id, open)| open.then_some(id))
        .collect()
}

/// The `HX-Retarget` header of an answer, if any.
fn retarget<B>(resp: &actix_web::dev::ServiceResponse<B>) -> Option<String> {
    resp.headers()
        .get("HX-Retarget")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}
