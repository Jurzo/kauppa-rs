//! Ingredient handlers (MB-0003, MB-0013): add, the match hint, quantity, move and remove.

use actix_identity::Identity;
use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use uuid::Uuid;

use kauppa_core::domain::{Direction, NewIngredient};
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::routes::products::parse_category;
use crate::routes::request::{current_user, wants_fragment};
use crate::state::AppState;

use super::{Draft, Section, match_hint, respond};

#[derive(Debug, Deserialize)]
struct IngredientForm {
    name: String,
    #[serde(default)]
    quantity: String,
    // Missing and empty both mean "no category chosen", which only matters
    // for a new product.
    #[serde(default)]
    category_id: String,
}

/// The name typed so far, for the match hint.
#[derive(Debug, Deserialize)]
struct MatchQuery {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct QuantityForm {
    #[serde(default)]
    quantity: String,
}

#[derive(Debug, Deserialize)]
struct MoveForm {
    dir: Direction,
}

/// Enforce "an ingredient needs a categorized product" (RR-0001, MB-0003 P2)
/// and build the repo input. An existing product must already have a
/// category; a new one needs a category chosen in the form. Whether the
/// category is the user's is checked by the repo.
async fn check_ingredient(
    state: &AppState,
    user: Uuid,
    form: &IngredientForm,
) -> CoreResult<NewIngredient> {
    let chosen = match form.category_id.trim() {
        "" => None,
        raw => Some(parse_category(raw)?),
    };
    let existing = ProductRepo::find_by_name(&state.pool, user, &form.name).await?;
    if let Some(product) = &existing {
        if product.category_id.is_none() {
            return Err(CoreError::Invalid(format!(
                "{} has no category — choose one for it on Favorites first",
                product.name
            )));
        }
    } else if chosen.is_none() {
        return Err(CoreError::Invalid(format!(
            "choose a category for the new product {}",
            form.name.trim()
        )));
    }
    Ok(NewIngredient {
        product_name: form.name.clone(),
        quantity: Some(form.quantity.clone()),
        category_id: chosen,
    })
}

#[post("/{id}/ingredients")]
pub async fn add_ingredient(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<IngredientForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let recipe_id = path.into_inner();
    let outcome = match check_ingredient(&state, user, &form).await {
        Ok(new) => RecipeRepo::add_ingredient(&state.pool, user, recipe_id, &new).await,
        Err(e) => Err(e),
    };
    let rejected = outcome.is_err();
    let draft = Draft::Ingredient {
        name: if rejected {
            form.name.clone()
        } else {
            String::new()
        },
        quantity: if rejected {
            form.quantity.clone()
        } else {
            String::new()
        },
        category_id: Uuid::parse_str(form.category_id.trim()).ok(),
    };
    respond(
        &state,
        &req,
        user,
        recipe_id,
        Section::Ingredients,
        outcome,
        draft,
    )
    .await
}

/// The match hint for the name typed into the add form, as a fragment for
/// htmx; otherwise a 303 back to the editor. Owner-scoped like the rest of
/// `/recipes/{id}`: another user's recipe is a 404.
#[get("/{id}/ingredients/match")]
pub async fn ingredient_match(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    query: web::Query<MatchQuery>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let recipe_id = path.into_inner();
    RecipeRepo::get(&state.pool, user, recipe_id).await?;
    if !wants_fragment(&req) {
        return Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, format!("/recipes/{recipe_id}")))
            .finish());
    }
    let categories = CategoryRepo::list_for_owner(&state.pool, user).await?;
    let hint = match_hint(&state, user, &query.name, &categories).await?;
    let body = state.templates.render("recipes/_match_hint", &hint)?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}

#[post("/{id}/ingredients/{ingredient}/quantity")]
pub async fn set_quantity(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
    form: web::Form<QuantityForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (recipe_id, ingredient_id) = path.into_inner();
    let outcome = RecipeRepo::set_ingredient_quantity(
        &state.pool,
        user,
        recipe_id,
        ingredient_id,
        Some(&form.quantity),
    )
    .await;
    respond(
        &state,
        &req,
        user,
        recipe_id,
        Section::Ingredients,
        outcome,
        Draft::None,
    )
    .await
}

/// Move an ingredient one step up or down within its recipe.
#[post("/{id}/ingredients/{ingredient}/move")]
pub async fn move_ingredient(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
    form: web::Form<MoveForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (recipe_id, ingredient_id) = path.into_inner();
    let outcome =
        RecipeRepo::move_ingredient(&state.pool, user, recipe_id, ingredient_id, form.dir).await;
    respond(
        &state,
        &req,
        user,
        recipe_id,
        Section::Ingredients,
        outcome,
        Draft::None,
    )
    .await
}

#[post("/{id}/ingredients/{ingredient}/delete")]
pub async fn remove_ingredient(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (recipe_id, ingredient_id) = path.into_inner();
    let outcome = RecipeRepo::remove_ingredient(&state.pool, user, recipe_id, ingredient_id).await;
    respond(
        &state,
        &req,
        user,
        recipe_id,
        Section::Ingredients,
        outcome,
        Draft::None,
    )
    .await
}
