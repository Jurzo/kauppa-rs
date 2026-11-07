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
//! lives in `ingredients::check_ingredient`. A new product takes the category chosen
//! in the form; an existing one keeps its own.
//!
//! MB-0013: the add form suggests the user's product names (a `<datalist>`)
//! and hints, as the name is typed, which category it will get
//! (`GET /recipes/{id}/ingredients/match`). Both are display hints only;
//! `check_ingredient` stays the one enforcement.
//!
//! The editor's third section, "Add to a list" (`#add-to-list`, MB-0015),
//! adds the recipe's ingredients to one of the user's lists through
//! `ShoppingListRepo::add_recipe`, as `POST /lists/{id}/recipes` (MB-0005)
//! does. htmx gets the section back with a confirmation; plain requests get a
//! 303 to the list.
//!
//! Split by area (MB-0019, RR-0009): this file has the views, rendering and
//! the recipe handlers (index, create, show, update, delete); `ingredients`
//! has the ingredient handlers and the match hint (MB-0003, MB-0013), and
//! `add_to_list` the "Add to a list" handler (MB-0015). Tests are in
//! `tests/<area>.rs`.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{Category, Recipe, RecipeDetail, RecipeInput, ShoppingList};
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::WebResult;
use crate::routes::products::Picker;
use crate::routes::request::{current_user, is_hx};
use crate::state::AppState;

mod add_to_list;
mod ingredients;
#[cfg(test)]
mod tests;

pub use add_to_list::add_recipe_to_list;
pub use ingredients::{
    add_ingredient, ingredient_match, move_ingredient, remove_ingredient, set_quantity,
};

#[derive(Debug, Deserialize)]
struct RecipeForm {
    title: String,
    #[serde(default)]
    instructions: String,
    #[serde(default)]
    servings: String,
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
    /// The user's product names for the ingredient name input's
    /// `<datalist>`. Rendered once per page load, outside the `#ingredients`
    /// fragment, so htmx edits don't re-send it.
    suggestions: Vec<String>,
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
    hint: MatchHint,
}

/// What the typed ingredient name will do (`recipes/_match_hint`), mirroring
/// `check_ingredient`: an existing product brings its own `category`, an
/// existing product without one (`uncategorized`, its name) can't be added,
/// and a `new_product` needs the picker. All empty for a blank name.
#[derive(Debug, Default, Serialize)]
struct MatchHint {
    category: Option<String>,
    uncategorized: Option<String>,
    new_product: bool,
}

/// The match hint for `name`, looked up through `ProductRepo::find_by_name`
/// (`name_key`, RR-0003). `categories` are the user's, to name the match's.
async fn match_hint(
    state: &AppState,
    user: Uuid,
    name: &str,
    categories: &[Category],
) -> CoreResult<MatchHint> {
    if name.trim().is_empty() {
        return Ok(MatchHint::default());
    }
    let hint = match ProductRepo::find_by_name(&state.pool, user, name).await? {
        None => MatchHint {
            new_product: true,
            ..MatchHint::default()
        },
        Some(product) => match product.category_id {
            None => MatchHint {
                uncategorized: Some(product.name),
                ..MatchHint::default()
            },
            // A category id that isn't among the user's can't happen (owner-
            // checked writes, `ON DELETE SET NULL`); it shows no hint.
            Some(id) => MatchHint {
                category: categories
                    .iter()
                    .find(|c| c.id == id)
                    .map(|c| c.name.clone()),
                ..MatchHint::default()
            },
        },
    };
    Ok(hint)
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
    let hint = match_hint(state, user, &name, &categories).await?;
    let add_section = add_to_list_view(recipe.id, lists, draft, added, add_error);

    Ok(ShowView {
        title: recipe.title.clone(),
        // Filled in by `render_show` for the full page only.
        suggestions: Vec::new(),
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
                hint,
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
    let mut view = show_view(state, user, detail, feedback).await?;
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
        None => {
            view.suggestions = ProductRepo::suggestion_names(&state.pool, user).await?;
            state.templates.render("recipes/show", &view)?
        }
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

/// Delete a recipe with its ingredients (MB-0012) and land on `/recipes`:
/// `HX-Redirect` for htmx, a 303 otherwise. Products stay in the catalog.
/// The editor asks for confirmation (`hx-confirm`) before posting.
#[post("/{id}/delete")]
pub async fn delete(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let recipe_id = path.into_inner();
    RecipeRepo::delete(&state.pool, user, recipe_id).await?;
    tracing::info!(user_id = %user, recipe_id = %recipe_id, "recipe deleted");
    Ok(if is_hx(&req) {
        HttpResponse::Ok()
            .append_header(("HX-Redirect", "/recipes"))
            .finish()
    } else {
        HttpResponse::SeeOther()
            .append_header((header::LOCATION, "/recipes"))
            .finish()
    })
}
