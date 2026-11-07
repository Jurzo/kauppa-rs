//! Tests for `routes::products`, by area (RR-0009). Fixtures shared by several areas live here.

mod catalog;
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
