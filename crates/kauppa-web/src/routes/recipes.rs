//! Recipes and their ingredients (MB-0003).
//!
//! Thin handlers: parse → call `RecipeRepo` → render. The editor at
//! `/recipes/{id}` has two htmx sections, the details form (`#recipe-details`)
//! and the ingredient list (`#ingredients`). htmx requests get the section's
//! re-rendered fragment; plain requests get a 303 back to the page (PRG), or
//! the full page when the input was rejected. All endpoints require an
//! authenticated user, and another user's recipe is a 404.
//!
//! RR-0001: an ingredient's product must have a category. The column is
//! nullable (deleting a category un-categorizes its products), so the rule
//! lives here, in `check_ingredient`. A new product takes the category chosen
//! in the form; an existing one keeps its own.
//!
//! The editor's third section, "Add to a list" (`#add-to-list`, MB-0015),
//! adds the recipe's ingredients to one of the user's lists through
//! `ShoppingListRepo::add_recipe`, as `POST /lists/{id}/recipes` (MB-0005)
//! does. htmx gets the section back with a confirmation; plain requests get a
//! 303 to the list.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{
    Direction, NewIngredient, Recipe, RecipeDetail, RecipeInput, ShoppingList,
};
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::{WebError, WebResult};
use crate::routes::lists::parse_multiplier;
use crate::routes::products::{Picker, parse_category};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct RecipeForm {
    title: String,
    #[serde(default)]
    instructions: String,
    #[serde(default)]
    servings: String,
}

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

#[derive(Debug, Deserialize)]
struct QuantityForm {
    #[serde(default)]
    quantity: String,
}

#[derive(Debug, Deserialize)]
struct MoveForm {
    dir: Direction,
}

#[derive(Debug, Deserialize)]
struct AddToListForm {
    // Missing and empty both mean "no list chosen", which becomes a message.
    #[serde(default)]
    list_id: String,
    #[serde(default)]
    multiplier: String,
}

fn current_user(id: &Identity) -> WebResult<Uuid> {
    let raw = id
        .id()
        .map_err(|_| WebError::Core(CoreError::Unauthorized))?;
    Uuid::parse_str(&raw).map_err(|_| WebError::Core(CoreError::Unauthorized))
}

fn is_hx(req: &HttpRequest) -> bool {
    req.headers().get("HX-Request").is_some_and(|v| v == "true")
}

/// An htmx request for part of a page. Boosted navigation (`hx-boost` on
/// `<body>`) also sends `HX-Request`, but swaps the whole body, so it gets the
/// full page (or follows the redirect).
fn wants_fragment(req: &HttpRequest) -> bool {
    is_hx(req) && req.headers().get("HX-Boosted").is_none()
}

/// The list chosen in the "Add to a list" form. Whether it is the user's is
/// checked by the repo.
fn parse_list(raw: &str) -> CoreResult<Uuid> {
    match raw.trim() {
        "" => Err(CoreError::Invalid("choose a list to add to".to_owned())),
        raw => Uuid::parse_str(raw).map_err(|_| CoreError::Invalid("unknown list".to_owned())),
    }
}

/// Parse the details form. Servings is optional; anything but a whole number
/// is rejected (the repo checks it is at least 1).
fn parse_recipe(form: &RecipeForm) -> CoreResult<RecipeInput> {
    let servings = form.servings.trim();
    let servings = if servings.is_empty() {
        None
    } else {
        Some(
            servings
                .parse::<i64>()
                .map_err(|_| CoreError::Invalid("servings must be a whole number".to_owned()))?,
        )
    };
    Ok(RecipeInput {
        title: form.title.clone(),
        instructions: form.instructions.clone(),
        servings,
    })
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

#[derive(Debug, Serialize)]
struct IndexView {
    recipes: Vec<RecipeSummary>,
    new: NewRecipeView,
}

#[derive(Debug, Serialize)]
struct RecipeSummary {
    id: Uuid,
    title: String,
    servings: Option<i64>,
}

/// The "new recipe" form (`recipes/_new`).
#[derive(Debug, Default, Serialize)]
struct NewRecipeView {
    title: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ShowView {
    title: String,
    details: DetailsView,
    ingredients: IngredientsView,
    add_to_list: AddToListView,
}

/// The details form (`recipes/_details`).
#[derive(Debug, Serialize)]
struct DetailsView {
    id: Uuid,
    title: String,
    instructions: String,
    servings: String,
    error: Option<String>,
    saved: bool,
}

/// The ingredient list and its "add" form (`recipes/_ingredients`).
#[derive(Debug, Serialize)]
struct IngredientsView {
    recipe_id: Uuid,
    items: Vec<IngredientRow>,
    has_categories: bool,
    add: AddIngredientView,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct IngredientRow {
    id: Uuid,
    name: String,
    quantity: String,
    category: Option<String>,
}

#[derive(Debug, Serialize)]
struct AddIngredientView {
    name: String,
    quantity: String,
    picker: Picker,
}

/// The "Add to a list" section (`recipes/_add_to_list`, MB-0015): the user's
/// lists, most recently updated first, and the confirmation of the last add.
#[derive(Debug, Serialize)]
struct AddToListView {
    recipe_id: Uuid,
    lists: Vec<ListOption>,
    has_lists: bool,
    /// Whether a list is preselected; otherwise the placeholder is.
    has_selection: bool,
    /// The multiplier as typed; the form shows 1 when empty.
    multiplier: String,
    added: Option<AddedView>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListOption {
    id: Uuid,
    name: String,
    selected: bool,
}

/// "Added 5 items to Viikonloppu — open list".
#[derive(Debug, Serialize)]
struct AddedView {
    list_id: Uuid,
    list_name: String,
    /// The count with its noun: "1 item", "5 items".
    items: String,
}

/// What a form shows after a rejected submission: the typed values, for
/// correction. The ingredient form also keeps its category after a success,
/// for quick entry of several products from one aisle.
#[derive(Debug, Default)]
enum Draft {
    #[default]
    None,
    Details(RecipeForm),
    Ingredient {
        name: String,
        quantity: String,
        category_id: Option<Uuid>,
    },
    /// The "Add to a list" form keeps its list after a success too.
    AddToList {
        list_id: Option<Uuid>,
        multiplier: String,
    },
}

/// What the editor shows besides the stored recipe: typed values after a
/// rejection, the error (in the section it belongs to), whether the details
/// were just saved, and the list and item count of a recipe just added to a
/// list.
#[derive(Debug, Default)]
struct Feedback {
    draft: Draft,
    error: Option<(Section, String)>,
    saved: bool,
    added: Option<(Uuid, usize)>,
}

/// The section of the editor a response is about.
#[derive(Debug, Clone, Copy)]
enum Section {
    Details,
    Ingredients,
    AddToList,
}

impl Section {
    const fn template(self) -> &'static str {
        match self {
            Self::Details => "recipes/_details",
            Self::Ingredients => "recipes/_ingredients",
            Self::AddToList => "recipes/_add_to_list",
        }
    }
}

async fn show_view(
    state: &AppState,
    user: Uuid,
    detail: RecipeDetail,
    feedback: Feedback,
) -> WebResult<ShowView> {
    let Feedback {
        draft,
        error,
        saved,
        added,
    } = feedback;
    let categories = CategoryRepo::list_for_owner(&state.pool, user).await?;
    let lists = ShoppingListRepo::list_for_owner(&state.pool, user).await?;
    let RecipeDetail {
        recipe,
        ingredients,
    } = detail;
    let (mut details_error, mut ingredients_error, mut add_error) = (None, None, None);
    match error {
        Some((Section::Details, m)) => details_error = Some(m),
        Some((Section::Ingredients, m)) => ingredients_error = Some(m),
        Some((Section::AddToList, m)) => add_error = Some(m),
        None => {}
    }

    let details = match &draft {
        Draft::Details(form) => DetailsView {
            id: recipe.id,
            title: form.title.clone(),
            instructions: form.instructions.clone(),
            servings: form.servings.clone(),
            error: details_error,
            saved: false,
        },
        _ => DetailsView {
            id: recipe.id,
            title: recipe.title.clone(),
            instructions: recipe.instructions.clone(),
            servings: recipe.servings.map(|n| n.to_string()).unwrap_or_default(),
            error: details_error,
            saved,
        },
    };
    let (name, quantity, category_id) = match &draft {
        Draft::Ingredient {
            name,
            quantity,
            category_id,
        } => (name.clone(), quantity.clone(), *category_id),
        _ => (String::new(), String::new(), None),
    };
    let add_section = add_to_list_view(recipe.id, lists, draft, added, add_error);

    Ok(ShowView {
        title: recipe.title.clone(),
        details,
        ingredients: IngredientsView {
            recipe_id: recipe.id,
            items: ingredients
                .into_iter()
                .map(|i| IngredientRow {
                    id: i.id,
                    name: i.text,
                    quantity: i.quantity.unwrap_or_default(),
                    category: i.category.map(|c| c.name),
                })
                .collect(),
            has_categories: !categories.is_empty(),
            add: AddIngredientView {
                name,
                quantity,
                picker: Picker::new(&categories, category_id).optional(),
            },
            error: ingredients_error,
        },
        add_to_list: add_section,
    })
}

fn add_to_list_view(
    recipe_id: Uuid,
    lists: Vec<ShoppingList>,
    draft: Draft,
    added: Option<(Uuid, usize)>,
    error: Option<String>,
) -> AddToListView {
    let (chosen, multiplier) = match draft {
        Draft::AddToList {
            list_id,
            multiplier,
        } => (list_id, multiplier),
        _ => (None, String::new()),
    };
    let added = added.map(|(list_id, count)| AddedView {
        list_id,
        // The list was just written to; a concurrent delete is the only way
        // it can be missing here.
        list_name: lists
            .iter()
            .find(|l| l.id == list_id)
            .map_or_else(|| "the list".to_owned(), |l| l.name.clone()),
        items: if count == 1 {
            "1 item".to_owned()
        } else {
            format!("{count} items")
        },
    });
    let lists: Vec<ListOption> = lists
        .into_iter()
        .map(|l| ListOption {
            selected: chosen == Some(l.id),
            id: l.id,
            name: l.name,
        })
        .collect();
    AddToListView {
        recipe_id,
        has_lists: !lists.is_empty(),
        has_selection: lists.iter().any(|l| l.selected),
        lists,
        multiplier,
        added,
        error,
    }
}

/// Render the editor as the full page or, for htmx, one section's fragment.
async fn render_show(
    state: &AppState,
    user: Uuid,
    recipe_id: Uuid,
    fragment: Option<Section>,
    status: StatusCode,
    feedback: Feedback,
) -> WebResult<HttpResponse> {
    let detail = RecipeRepo::get(&state.pool, user, recipe_id).await?;
    let view = show_view(state, user, detail, feedback).await?;
    let body = match fragment {
        Some(Section::Details) => state
            .templates
            .render(Section::Details.template(), &view.details)?,
        Some(Section::Ingredients) => state
            .templates
            .render(Section::Ingredients.template(), &view.ingredients)?,
        Some(Section::AddToList) => state
            .templates
            .render(Section::AddToList.template(), &view.add_to_list)?,
        None => state.templates.render("recipes/show", &view)?,
    };
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// Answer an edit of one section. Rejected input (`Invalid` / `Conflict`) is
/// shown to the user: in the section's fragment for htmx — with 200, since
/// htmx doesn't swap 4xx responses by default — or on the full page with the
/// 4xx status. Success is the fragment for htmx, else a 303 back to the page.
/// `NotFound` (another user's recipe or ingredient) is a 404.
async fn respond<T>(
    state: &AppState,
    req: &HttpRequest,
    user: Uuid,
    recipe_id: Uuid,
    section: Section,
    outcome: CoreResult<T>,
    draft: Draft,
) -> WebResult<HttpResponse> {
    let (status, error) = match outcome {
        Ok(_) => (StatusCode::OK, None),
        Err(CoreError::Invalid(m)) => (StatusCode::BAD_REQUEST, Some(m)),
        Err(CoreError::Conflict(m)) => (StatusCode::CONFLICT, Some(m)),
        Err(e) => return Err(e.into()),
    };
    if let Some(reason) = &error {
        tracing::debug!(user_id = %user, recipe_id = %recipe_id, %reason, "recipe input rejected");
    }
    let feedback = Feedback {
        draft,
        saved: error.is_none() && matches!(section, Section::Details),
        error: error.map(|m| (section, m)),
        added: None,
    };

    if is_hx(req) {
        render_show(
            state,
            user,
            recipe_id,
            Some(section),
            StatusCode::OK,
            feedback,
        )
        .await
    } else if feedback.error.is_none() {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, format!("/recipes/{recipe_id}")))
            .finish())
    } else {
        render_show(state, user, recipe_id, None, status, feedback).await
    }
}

fn summaries(recipes: Vec<Recipe>) -> Vec<RecipeSummary> {
    recipes
        .into_iter()
        .map(|r| RecipeSummary {
            id: r.id,
            title: r.title,
            servings: r.servings,
        })
        .collect()
}

#[get("")]
pub async fn index(state: web::Data<AppState>, id: Identity) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let recipes = RecipeRepo::list_for_owner(&state.pool, user).await?;
    let view = IndexView {
        recipes: summaries(recipes),
        new: NewRecipeView::default(),
    };
    let body = state.templates.render("recipes/index", &view)?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}

/// Create a recipe from its title and open it in the editor: `HX-Redirect`
/// for htmx, a 303 otherwise. A rejected title re-renders the form.
#[post("")]
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    form: web::Form<RecipeForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = match parse_recipe(&form) {
        Ok(input) => RecipeRepo::create(&state.pool, user, &input).await,
        Err(e) => Err(e),
    };
    let (status, message) = match outcome {
        Ok(recipe) => {
            let location = format!("/recipes/{}", recipe.id);
            return Ok(if is_hx(&req) {
                HttpResponse::Ok()
                    .append_header(("HX-Redirect", location))
                    .finish()
            } else {
                HttpResponse::SeeOther()
                    .append_header((header::LOCATION, location))
                    .finish()
            });
        }
        Err(CoreError::Invalid(m)) => (StatusCode::BAD_REQUEST, m),
        Err(e) => return Err(e.into()),
    };
    tracing::debug!(user_id = %user, reason = %message, "new recipe rejected");

    let new = NewRecipeView {
        title: form.title.clone(),
        error: Some(message),
    };
    let body = if is_hx(&req) {
        state.templates.render("recipes/_new", &new)?
    } else {
        let recipes = RecipeRepo::list_for_owner(&state.pool, user).await?;
        let view = IndexView {
            recipes: summaries(recipes),
            new,
        };
        state.templates.render("recipes/index", &view)?
    };
    let status = if is_hx(&req) { StatusCode::OK } else { status };
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

#[get("/{id}")]
pub async fn show(
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    render_show(
        &state,
        user,
        path.into_inner(),
        None,
        StatusCode::OK,
        Feedback::default(),
    )
    .await
}

#[post("/{id}")]
pub async fn update(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<RecipeForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let recipe_id = path.into_inner();
    let outcome = match parse_recipe(&form) {
        Ok(input) => RecipeRepo::update(&state.pool, user, recipe_id, &input).await,
        Err(e) => Err(e),
    };
    let draft = if outcome.is_ok() {
        Draft::None
    } else {
        Draft::Details(form.into_inner())
    };
    respond(
        &state,
        &req,
        user,
        recipe_id,
        Section::Details,
        outcome,
        draft,
    )
    .await
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

/// Add the recipe's ingredients to one of the user's lists (MB-0015), as
/// `POST /lists/{id}/recipes` does: the same snapshot and multiplier rules,
/// through `ShoppingListRepo::add_recipe`. htmx gets the section back with a
/// confirmation, plain requests a 303 to the list. A missing list, a bad
/// multiplier, or a recipe without ingredients is shown in the section (200
/// over htmx, else the full page with 400); another user's list or recipe is
/// a 404.
#[post("/{id}/add-to-list")]
pub async fn add_recipe_to_list(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<AddToListForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let recipe_id = path.into_inner();
    let form = form.into_inner();
    let outcome = match (
        parse_list(&form.list_id),
        parse_multiplier(&form.multiplier),
    ) {
        (Ok(list_id), Ok(multiplier)) => {
            ShoppingListRepo::add_recipe(&state.pool, user, list_id, recipe_id, multiplier)
                .await
                .map(|items| (list_id, items.len()))
        }
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    let fragment = wants_fragment(&req);
    let (status, feedback) = match outcome {
        Ok((list_id, _)) if !fragment => {
            return Ok(HttpResponse::SeeOther()
                .append_header((header::LOCATION, format!("/lists/{list_id}")))
                .finish());
        }
        Ok((list_id, count)) => (
            StatusCode::OK,
            Feedback {
                draft: Draft::AddToList {
                    list_id: Some(list_id),
                    multiplier: String::new(),
                },
                added: Some((list_id, count)),
                ..Feedback::default()
            },
        ),
        Err(CoreError::Invalid(reason)) => {
            tracing::debug!(user_id = %user, recipe_id = %recipe_id, %reason, "add to list rejected");
            (
                StatusCode::BAD_REQUEST,
                Feedback {
                    draft: Draft::AddToList {
                        list_id: Uuid::parse_str(form.list_id.trim()).ok(),
                        multiplier: form.multiplier,
                    },
                    error: Some((Section::AddToList, reason)),
                    ..Feedback::default()
                },
            )
        }
        Err(e) => return Err(e.into()),
    };
    if fragment {
        let section = Some(Section::AddToList);
        render_show(&state, user, recipe_id, section, StatusCode::OK, feedback).await
    } else {
        render_show(&state, user, recipe_id, None, status, feedback).await
    }
}

// MB-0003 acceptance tests (P2, P6). They sit at module level rather than in a
// `tests` submodule so the brief's `cargo test -p kauppa-web
// routes::recipes::<name>` checks select them.

/// Ingredient names of a recipe, in order.
#[cfg(test)]
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

#[cfg(test)]
#[allow(clippy::expect_used)]
async fn make_recipe(state: &AppState, user: Uuid, title: &str) -> Uuid {
    let input = RecipeInput {
        title: title.to_owned(),
        ..RecipeInput::default()
    };
    RecipeRepo::create(&state.pool, user, &input)
        .await
        .expect("create recipe")
        .id
}

/// P2: the server refuses an ingredient whose product has no category — an
/// existing product left uncategorized by a category deletion, or a new name
/// without a chosen category — even when a client skips the form, and adds
/// nothing. A categorized product is accepted.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn ingredient_requires_categorized_product() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let juomat = make_category(&state, user, "Juomat").await;
    let theirs = make_category(&state, other, "Pakasteet").await;
    let recipe = make_recipe(&state, user, "Aamiainen").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    // Uncategorized products: a favorite and a recipe-created (hidden) one
    // whose category was deleted.
    ProductRepo::create(&state.pool, user, "Kahvi", juomat)
        .await
        .expect("create favorite");
    let scratch = make_recipe(&state, user, "Luonnos").await;
    RecipeRepo::add_ingredient(
        &state.pool,
        user,
        scratch,
        &NewIngredient {
            product_name: "Mehu".to_owned(),
            quantity: None,
            category_id: Some(juomat),
        },
    )
    .await
    .expect("create hidden product");
    CategoryRepo::delete(&state.pool, user, juomat)
        .await
        .expect("delete category");

    let submit = |form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/ingredients"))
            .cookie(session.clone())
            .set_form(form)
    };
    let field = |k: &'static str, v: &str| (k, v.to_owned());

    let rejected = [
        (
            vec![
                field("name", "kahvi"),
                field("category_id", &hevi.to_string()),
            ],
            "has no category",
        ),
        (vec![field("name", "MEHU")], "has no category"),
        (vec![field("name", "Kaura")], "choose a category"),
        (
            vec![field("name", "Kaura"), field("category_id", "")],
            "choose a category",
        ),
        (
            vec![field("name", "Kaura"), field("category_id", "not-a-uuid")],
            "unknown category",
        ),
        (
            vec![
                field("name", "Kaura"),
                field("category_id", &theirs.to_string()),
            ],
            "unknown category",
        ),
    ];
    for (form, message) in rejected {
        let resp = call_service(&app, submit(form.clone()).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{form:?}");
        let body = body_string(resp).await;
        assert!(body.contains("<html"), "full page without htmx");
        assert!(body.contains(message), "{form:?} should say {message:?}");
    }
    assert_eq!(
        ingredient_names(&state, user, recipe).await,
        Vec::<String>::new()
    );
    assert!(
        ProductRepo::find_by_name(&state.pool, user, "Kaura")
            .await
            .expect("lookup")
            .is_none()
    );
    let kahvi = ProductRepo::find_by_name(&state.pool, user, "Kahvi")
        .await
        .expect("lookup")
        .expect("exists");
    assert_eq!(kahvi.category_id, None, "a rejection doesn't recategorize");

    // Over htmx the rejection is the fragment, keeping the typed values.
    let resp = call_service(
        &app,
        submit(vec![field("name", "Kaura"), field("quantity", "1 dl")])
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html"), "fragment only");
    assert!(body.contains(r#"id="ingredients""#));
    assert!(body.contains("choose a category for the new product Kaura"));
    assert!(body.contains(r#"value="Kaura""#) && body.contains(r#"value="1 dl""#));

    // Control: a categorized product, existing or new, is accepted.
    for form in [
        vec![
            field("name", "Kaura"),
            field("category_id", &hevi.to_string()),
        ],
        vec![field("name", "kaura"), field("quantity", "2 dl")],
    ] {
        let resp = call_service(&app, submit(form.clone()).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{form:?}");
    }
    assert_eq!(
        ingredient_names(&state, user, recipe).await,
        ["Kaura", "Kaura"]
    );
}

/// P6: `GET /recipes/{id}` renders a recipe with its ingredients, create and
/// edit work over htmx (fragments and `HX-Redirect`) and without it (303),
/// and another user can neither read nor change the recipe.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    make_user(&state, "other@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let post = |uri: String, form: Vec<(&'static str, String)>| {
        TestRequest::post().uri(&uri).set_form(form)
    };

    // Create over htmx: an HX-Redirect into the editor.
    let resp = call_service(
        &app,
        post(
            "/recipes".to_owned(),
            vec![("title", "Pannukakku".to_owned())],
        )
        .cookie(session.clone())
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let location = resp
        .headers()
        .get("HX-Redirect")
        .expect("HX-Redirect")
        .to_str()
        .expect("ascii")
        .to_owned();
    let recipe: Uuid = location
        .strip_prefix("/recipes/")
        .expect("editor url")
        .parse()
        .expect("recipe id");

    // Without htmx: a 303 to the editor. An empty title re-renders the form.
    let resp = call_service(
        &app,
        post("/recipes".to_owned(), vec![("title", "Puuro".to_owned())])
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = call_service(
        &app,
        post("/recipes".to_owned(), vec![("title", " ".to_owned())])
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    let body = body_string(resp).await;
    assert!(body.contains(r#"id="new-recipe""#) && body.contains("must not be empty"));

    // Edit over htmx: each section answers with its own fragment.
    let resp = call_service(
        &app,
        post(
            format!("/recipes/{recipe}"),
            vec![
                ("title", "Pannukakku".to_owned()),
                ("servings", "4".to_owned()),
                ("instructions", "Paista uunissa.".to_owned()),
            ],
        )
        .cookie(session.clone())
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(r#"id="recipe-details""#));
    assert!(body.contains("Saved."));
    let resp = call_service(
        &app,
        post(
            format!("/recipes/{recipe}/ingredients"),
            vec![
                ("name", "Maito".to_owned()),
                ("quantity", "5 dl".to_owned()),
                ("category_id", kylma.to_string()),
            ],
        )
        .cookie(session.clone())
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(r#"id="ingredients""#));
    assert!(body.contains("Maito"));

    // The page renders the recipe with its ingredients and their categories.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{recipe}"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    for expected in [
        "<html",
        "Pannukakku",
        "Paista uunissa.",
        "5 dl",
        "Maito",
        "Kylmä",
    ] {
        assert!(body.contains(expected), "page should show {expected:?}");
    }
    let milk = RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe")
        .ingredients[0]
        .id;

    // Another user: every read and write is a 404, and nothing changes.
    let base = format!("/recipes/{recipe}");
    let item = format!("{base}/ingredients/{milk}");
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&base)
            .cookie(intruder.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let writes = [
        (base.clone(), vec![("title", "Kaapattu".to_owned())]),
        (
            format!("{base}/ingredients"),
            vec![
                ("name", "Omena".to_owned()),
                ("category_id", kylma.to_string()),
            ],
        ),
        (
            format!("{item}/quantity"),
            vec![("quantity", "1 l".to_owned())],
        ),
        (format!("{item}/move"), vec![("dir", "down".to_owned())]),
        (format!("{item}/delete"), vec![]),
    ];
    for (uri, form) in writes {
        for hx in [false, true] {
            let mut req = post(uri.clone(), form.clone()).cookie(intruder.clone());
            if hx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri} (htmx: {hx})");
        }
    }
    let their_index = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/recipes")
                .cookie(intruder.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(!their_index.contains(&base), "not listed for another user");

    let detail = RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe");
    assert_eq!(detail.recipe.title, "Pannukakku");
    assert_eq!(detail.ingredients.len(), 1);
    assert_eq!(detail.ingredients[0].quantity.as_deref(), Some("5 dl"));

    // Signed out: no access at all.
    let resp = call_service(&app, TestRequest::get().uri(&base).to_request()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// MB-0016 acceptance tests (P2–P4), at module level for the brief's
// `cargo test -p kauppa-web routes::recipes::<name>` checks. They read full
// pages, where the layout wraps the body.

/// MB-0016 P2: the `/recipes` index shows each card with its own title; the
/// page title is still "Recipes".
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn index_shows_titles() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let pancakes = make_recipe(&state, user, "Pannukakku").await;
    let porridge = make_recipe(&state, user, "Puuro").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/recipes")
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Recipes — kauppa</title>"));
    for (id, title) in [(pancakes, "Pannukakku"), (porridge, "Puuro")] {
        let card = format!(r#"<a href="/recipes/{id}">{title}</a>"#);
        assert!(body.contains(&card), "{card}");
    }
}

/// MB-0016 P3: the new-recipe title field is empty on a normal load, and
/// after a rejected submission without htmx it keeps what the user typed.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn new_recipe_draft() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "u@example.com").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    // The new-recipe title input; its placeholder is unique on the page. (The
    // tag spans two lines, re-indented where the partial is included.)
    let input = |value: &str| format!(r#"placeholder="e.g. Pannukakku" value="{value}" />"#);

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/recipes")
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Recipes — kauppa</title>"));
    assert!(body.contains(&input("")), "empty on a normal load");

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/recipes")
            .cookie(session)
            .set_form([("title", " ")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Recipes — kauppa</title>"));
    assert!(body.contains("must not be empty"));
    assert!(body.contains(&input(" ")), "keeps the typed value");
}

/// MB-0016 P4: after a rejected details edit without htmx, the title field
/// holds the typed title; the page heading still shows the stored one.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn details_draft_survives_rejection() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let recipe = make_recipe(&state, user, "Pannukakku").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}"))
            .cookie(session)
            .set_form([("title", "Uusi nimi"), ("servings", "abc")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Pannukakku — kauppa</title>"));
    assert!(body.contains("<h1>Pannukakku</h1>"));
    assert!(
        body.contains(r#"<input name="title" type="text" required value="Uusi nimi" />"#),
        "keeps the typed title"
    );
    assert_eq!(
        RecipeRepo::get(&state.pool, user, recipe)
            .await
            .expect("get recipe")
            .recipe
            .title,
        "Pannukakku",
        "nothing saved"
    );
}

// MB-0015 acceptance tests (P1–P3), at module level for the brief's
// `cargo test -p kauppa-web routes::recipes::<name>` checks. As a filter,
// `routes::recipes::add_to_list` also selects the other two.

/// A recipe with one ingredient per `(product name, quantity)`, all in
/// `category`.
#[cfg(test)]
#[allow(clippy::expect_used)]
async fn make_recipe_with(
    state: &AppState,
    owner: Uuid,
    title: &str,
    category: Uuid,
    ingredients: &[(&str, Option<&str>)],
) -> Uuid {
    let recipe = make_recipe(state, owner, title).await;
    for &(name, quantity) in ingredients {
        let new = NewIngredient {
            product_name: name.to_owned(),
            quantity: quantity.map(str::to_owned),
            category_id: Some(category),
        };
        RecipeRepo::add_ingredient(&state.pool, owner, recipe, &new)
            .await
            .expect("add ingredient");
    }
    recipe
}

/// A list's active items as `(text, quantity, product, category)`.
#[cfg(test)]
#[allow(clippy::expect_used)]
async fn list_shape(
    state: &AppState,
    owner: Uuid,
    list: Uuid,
) -> Vec<(String, Option<String>, Option<Uuid>, Option<Uuid>)> {
    ShoppingListRepo::items(&state.pool, owner, list, false)
        .await
        .expect("list items")
        .into_iter()
        .map(|i| (i.text, i.quantity, i.product_id, i.category.map(|c| c.id)))
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
async fn make_list(state: &AppState, owner: Uuid, name: &str) -> Uuid {
    ShoppingListRepo::create(&state.pool, owner, name)
        .await
        .expect("create list")
        .id
}

/// MB-0015 P1: the recipe page offers the user's own lists, most recently
/// updated first, with a "Times" field defaulting to 1; another user's lists
/// never appear. Without lists, it links to `/lists` instead.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_to_list_form() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let fresh = make_user(&state, "fresh@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let recipe = make_recipe_with(&state, user, "Puuro", kylma, &[("Maito", Some("5 dl"))]).await;
    let fresh_category = make_category(&state, fresh, "Kuivat").await;
    let fresh_recipe = make_recipe_with(
        &state,
        fresh,
        "Leipä",
        fresh_category,
        &[("Jauho", Some("1 kg"))],
    )
    .await;
    let weekend = make_list(&state, user, "Viikonloppu").await;
    let weekday = make_list(&state, user, "Arki").await;
    let theirs = make_list(&state, other, "Heidän lista").await;
    // A rename counts as an update: the older list moves to the top.
    ShoppingListRepo::rename(&state.pool, user, weekend, "Viikonloppu")
        .await
        .expect("rename");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let fresh_session =
        session_cookie(&call_service(&app, login_request("fresh@example.com").to_request()).await);

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{recipe}"))
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Puuro — kauppa</title>"), "full page");
    assert!(body.contains(&format!(r#"action="/recipes/{recipe}/add-to-list""#)));
    assert!(body.contains(r#"<option value="" disabled selected>Choose a list</option>"#));
    let option = |id: Uuid, name: &str| format!(r#"<option value="{id}">{name}</option>"#);
    let first = body
        .find(&option(weekend, "Viikonloppu"))
        .expect("most recently updated list offered");
    let second = body
        .find(&option(weekday, "Arki"))
        .expect("other own list offered");
    assert!(first < second, "most recently updated first");
    assert!(
        body.contains(r#"<input name="multiplier" type="text" inputmode="decimal""#)
            && body.contains(r#"value="1" />"#),
        "Times defaults to 1"
    );
    assert!(!body.contains("Heidän lista") && !body.contains(&theirs.to_string()));

    // Without lists: a link to /lists, no form.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{fresh_recipe}"))
            .cookie(fresh_session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<title>Leipä — kauppa</title>"), "full page");
    assert!(body.contains(r#"<a href="/lists">create a list</a>"#));
    assert!(!body.contains(r#"name="list_id""#));
    assert!(!body.contains("Viikonloppu") && !body.contains("Heidän lista"));
}

/// MB-0015 P2: submitting adds the recipe's ingredients to the chosen list
/// exactly as `POST /lists/{id}/recipes` does (snapshot, multiplier). Over
/// htmx the answer is a confirmation naming the list and the item count, with
/// a link to it; without htmx (or boosted), a 303 to the list.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_to_list() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let recipe = make_recipe_with(
        &state,
        user,
        "Pannukakku",
        kylma,
        &[
            ("Maito", Some("5 dl")),
            ("Vehnäjauho", Some("2½ dl")),
            ("Kananmuna", None),
        ],
    )
    .await;
    let single = make_recipe_with(&state, user, "Kahvi", kylma, &[("Kerma", Some("1 dl"))]).await;
    let weekend = make_list(&state, user, "Viikonloppu").await;
    let weekday = make_list(&state, user, "Arki").await;
    let reference = make_list(&state, user, "Vertailu").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let submit = |recipe: Uuid, list: Uuid, multiplier: &str| {
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/add-to-list"))
            .cookie(session.clone())
            .set_form([
                ("list_id", list.to_string()),
                ("multiplier", multiplier.to_owned()),
            ])
    };

    // Over htmx: the section with a confirmation, the list still chosen.
    let resp = call_service(
        &app,
        submit(recipe, weekend, "1,5")
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html"), "fragment only");
    assert!(body.contains(r#"<div id="add-to-list""#));
    assert!(body.contains(&format!(
        r#"Added 3 items to Viikonloppu — <a href="/lists/{weekend}">open list</a>"#
    )));
    assert!(body.contains(&format!(
        r#"<option value="{weekend}" selected>Viikonloppu</option>"#
    )));

    // The same as adding it from the list page.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{reference}/recipes"))
            .cookie(session.clone())
            .set_form([
                ("recipe_id", recipe.to_string()),
                ("multiplier", "1,5".to_owned()),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let added = list_shape(&state, user, weekend).await;
    assert_eq!(added, list_shape(&state, user, reference).await);
    let quantities: Vec<_> = added.iter().map(|i| i.1.as_deref()).collect();
    assert_eq!(quantities, [Some("7.5 dl"), Some("3.75 dl"), None]);
    let detail = RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe");
    assert_eq!(added.len(), detail.ingredients.len());
    for (item, ingredient) in added.iter().zip(&detail.ingredients) {
        assert_eq!(item.0, ingredient.text);
        assert_eq!(item.2, ingredient.product_id);
        assert_eq!(item.3, Some(kylma));
    }

    // One ingredient: "1 item".
    let resp = call_service(
        &app,
        submit(single, weekday, "")
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    let body = body_string(resp).await;
    assert!(body.contains("Added 1 item to Arki"));

    // Without htmx, and boosted: a 303 to the list; blank means 1.
    for boosted in [false, true] {
        let mut req = submit(recipe, weekday, "");
        if boosted {
            req = req
                .insert_header(("HX-Request", "true"))
                .insert_header(("HX-Boosted", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "boosted: {boosted}");
        assert_eq!(
            resp.headers()
                .get(header::LOCATION)
                .expect("location")
                .to_str()
                .expect("ascii"),
            format!("/lists/{weekday}")
        );
    }
    let texts: Vec<_> = list_shape(&state, user, weekday)
        .await
        .into_iter()
        .map(|i| (i.0, i.1))
        .collect();
    let verbatim = [
        ("Kerma", Some("1 dl")),
        ("Maito", Some("5 dl")),
        ("Vehnäjauho", Some("2½ dl")),
        ("Kananmuna", None),
        ("Maito", Some("5 dl")),
        ("Vehnäjauho", Some("2½ dl")),
        ("Kananmuna", None),
    ]
    .map(|(t, q)| (t.to_owned(), q.map(str::to_owned)));
    assert_eq!(texts, verbatim);

    // Signed out: rejected, nothing added.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/recipes/{recipe}/add-to-list"))
            .set_form([("list_id", weekend.to_string())])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(list_shape(&state, user, weekend).await.len(), 3);
}

/// MB-0015 P3: another user's list or recipe is a 404 and adds nothing; a
/// missing list choice, a bad multiplier, or a recipe without ingredients is
/// a message — in the fragment over htmx, on the full page without it.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_to_list_rejections() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let mine = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "Kylmä").await;
    let my_recipe = make_recipe_with(&state, user, "Puuro", mine, &[("Maito", Some("5 dl"))]).await;
    let empty = make_recipe(&state, user, "Tyhjä").await;
    let their_recipe = make_recipe_with(
        &state,
        other,
        "Salaatti",
        theirs,
        &[("Feta", Some("1 pkt"))],
    )
    .await;
    let my_list = make_list(&state, user, "Oma").await;
    let their_list = make_list(&state, other, "Heidän").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let submit = |recipe: Uuid, form: Vec<(&'static str, String)>, hx: bool| {
        let mut req = TestRequest::post()
            .uri(&format!("/recipes/{recipe}/add-to-list"))
            .cookie(session.clone())
            .set_form(form);
        if hx {
            req = req.insert_header(("HX-Request", "true"));
        }
        req.to_request()
    };
    let list = |id: Uuid| ("list_id", id.to_string());
    let times = |m: &str| ("multiplier", m.to_owned());

    // Another user's list or recipe (or one that doesn't exist): 404, even
    // with otherwise bad input.
    let foreign = [
        (my_recipe, vec![list(their_list), times("2")]),
        (their_recipe, vec![list(my_list), times("2")]),
        (their_recipe, vec![list(their_list), times("2")]),
        (their_recipe, vec![times("kaksi")]),
        (my_recipe, vec![list(Uuid::new_v4()), times("2")]),
        (Uuid::new_v4(), vec![list(my_list), times("2")]),
    ];
    for (recipe, form) in foreign {
        for hx in [false, true] {
            let resp = call_service(&app, submit(recipe, form.clone(), hx)).await;
            assert_eq!(
                resp.status(),
                StatusCode::NOT_FOUND,
                "{form:?} (htmx: {hx})"
            );
        }
    }

    // Rejected input: the message, keeping the typed multiplier and list.
    let rejected = [
        (my_recipe, vec![times("2")], "choose a list to add to"),
        (
            my_recipe,
            vec![("list_id", String::new())],
            "choose a list to add to",
        ),
        (
            my_recipe,
            vec![("list_id", "abc".to_owned())],
            "unknown list",
        ),
        (
            my_recipe,
            vec![list(my_list), times("kaksi")],
            "the multiplier must be a number",
        ),
        (
            my_recipe,
            vec![list(my_list), times("0")],
            "the multiplier must be more than 0 and at most 100",
        ),
        (
            my_recipe,
            vec![list(my_list), times("101")],
            "the multiplier must be more than 0 and at most 100",
        ),
        (
            empty,
            vec![list(my_list), times("2")],
            "Tyhjä has no ingredients",
        ),
    ];
    for (recipe, form, message) in rejected {
        let resp = call_service(&app, submit(recipe, form.clone(), true)).await;
        assert_eq!(resp.status(), StatusCode::OK, "{form:?} over htmx");
        let body = body_string(resp).await;
        assert!(!body.contains("<html"), "fragment only");
        assert!(body.contains(r#"<div id="add-to-list""#));
        assert!(body.contains(message), "{form:?} should say {message:?}");
        if let Some((_, typed)) = form.iter().find(|(k, _)| *k == "multiplier") {
            assert!(
                body.contains(&format!(r#"value="{typed}" />"#)),
                "keeps the typed multiplier"
            );
        }
        if form.contains(&list(my_list)) {
            let chosen = format!(r#"<option value="{my_list}" selected>Oma</option>"#);
            assert!(body.contains(&chosen), "keeps the chosen list");
        }

        let resp = call_service(&app, submit(recipe, form.clone(), false)).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{form:?}");
        let body = body_string(resp).await;
        assert!(body.contains("<html") && body.contains("<h2>Add to a list</h2>"));
        assert!(
            body.contains(message),
            "{form:?} should say {message:?} on the page"
        );
    }

    // Nothing was added anywhere.
    assert_eq!(list_shape(&state, user, my_list).await, Vec::new());
    assert_eq!(list_shape(&state, other, their_list).await, Vec::new());
}
