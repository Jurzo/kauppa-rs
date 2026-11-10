//! Ingredient handlers (MB-0003, MB-0013, MB-0039): add (asking a new
//! product for its category), the match hint, quantity, move and remove.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::Deserialize;
use uuid::Uuid;

use kauppa_core::domain::{Direction, NewIngredient};
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::routes::products::parse_category;
use crate::routes::request::{current_user, is_hx, wants_fragment};
use crate::state::AppState;

use super::{Draft, Feedback, Section, match_hint, render_show, respond};

#[derive(Debug, Deserialize)]
struct IngredientForm {
    name: String,
    #[serde(default)]
    quantity: String,
    // Sent by the category prompt (MB-0039). Missing and empty both mean
    // "no category chosen", which only matters for a new product.
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

/// What `check_ingredient` found.
enum Checked {
    /// The repo input, ready to add.
    Ready(NewIngredient),
    /// A new product and no category chosen yet: ask for one (MB-0039 P2).
    NeedsCategory,
}

/// Enforce "an ingredient needs a categorized product" (RR-0001, MB-0003 P2)
/// and build the repo input. An existing product must already have a
/// category; a new one needs a category, asked for after the name is
/// submitted. Whether the category is the user's is checked by the repo,
/// which also refuses a new product without one.
async fn check_ingredient(
    state: &AppState,
    user: Uuid,
    form: &IngredientForm,
) -> CoreResult<Checked> {
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
        return Ok(Checked::NeedsCategory);
    }
    Ok(Checked::Ready(NewIngredient {
        product_name: form.name.clone(),
        quantity: Some(form.quantity.clone()),
        category_id: chosen,
    }))
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
        Ok(Checked::Ready(new)) => {
            RecipeRepo::add_ingredient(&state.pool, user, recipe_id, &new).await
        }
        Ok(Checked::NeedsCategory) => {
            return ask_category(&state, &req, user, recipe_id, form.into_inner()).await;
        }
        Err(e) => Err(e),
    };
    let draft = if outcome.is_ok() {
        Draft::None
    } else {
        // Rejected: keep the typed values, and the prompt while the name is
        // a new product (an unknown category chosen there).
        let form = form.into_inner();
        Draft::Ingredient {
            category_id: Uuid::parse_str(form.category_id.trim()).ok(),
            name: form.name,
            quantity: form.quantity,
            ask_category: true,
        }
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

/// Answer a new product's name with the category prompt (MB-0039 P2): the
/// editor, or its `#ingredients` fragment for htmx, keeping the name and
/// quantity. 200 either way: it is a question, not a rejection, and htmx
/// doesn't swap 4xx. Nothing is written.
async fn ask_category(
    state: &AppState,
    req: &HttpRequest,
    user: Uuid,
    recipe_id: Uuid,
    form: IngredientForm,
) -> WebResult<HttpResponse> {
    tracing::debug!(user_id = %user, recipe_id = %recipe_id, "new product: asking for its category");
    let feedback = Feedback {
        draft: Draft::Ingredient {
            name: form.name,
            quantity: form.quantity,
            category_id: None,
            ask_category: true,
        },
        ..Feedback::default()
    };
    let fragment = is_hx(req).then_some(Section::Ingredients);
    render_show(state, user, recipe_id, fragment, StatusCode::OK, feedback).await
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
