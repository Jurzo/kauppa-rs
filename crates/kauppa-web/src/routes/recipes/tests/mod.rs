//! Tests for `routes::recipes`, by area (RR-0009). Fixtures shared by several areas live here.

mod add_to_list;
mod delete;
mod editor;
mod pages;
mod suggestions;

use uuid::Uuid;

use kauppa_core::repo::RecipeRepo;

use crate::state::AppState;

/// Ingredient names of a recipe, in order.
#[allow(clippy::expect_used)]
async fn ingredient_names(state: &AppState, user: Uuid, recipe: Uuid) -> Vec<String> {
    RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe")
        .ingredients
        .into_iter()
        .map(|i| i.text)
        .collect()
}
