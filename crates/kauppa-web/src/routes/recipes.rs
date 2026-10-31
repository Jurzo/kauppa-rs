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

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{Direction, NewIngredient, Recipe, RecipeDetail, RecipeInput};
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::{WebError, WebResult};
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

fn current_user(id: &Identity) -> WebResult<Uuid> {
    let raw = id
        .id()
        .map_err(|_| WebError::Core(CoreError::Unauthorized))?;
    Uuid::parse_str(&raw).map_err(|_| WebError::Core(CoreError::Unauthorized))
}

fn is_hx(req: &HttpRequest) -> bool {
    req.headers().get("HX-Request").is_some_and(|v| v == "true")
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
}

/// What the editor shows besides the stored recipe: typed values after a
/// rejection, the error (in the section it belongs to), and whether the
/// details were just saved.
#[derive(Debug, Default)]
struct Feedback {
    draft: Draft,
    error: Option<(Section, String)>,
    saved: bool,
}

/// The section of the editor a response is about.
#[derive(Debug, Clone, Copy)]
enum Section {
    Details,
    Ingredients,
}

impl Section {
    const fn template(self) -> &'static str {
        match self {
            Self::Details => "recipes/_details",
            Self::Ingredients => "recipes/_ingredients",
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
    } = feedback;
    let categories = CategoryRepo::list_for_owner(&state.pool, user).await?;
    let RecipeDetail {
        recipe,
        ingredients,
    } = detail;
    let (details_error, ingredients_error) = match error {
        Some((Section::Details, m)) => (Some(m), None),
        Some((Section::Ingredients, m)) => (None, Some(m)),
        None => (None, None),
    };

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
    let (name, quantity, category_id) = match draft {
        Draft::Ingredient {
            name,
            quantity,
            category_id,
        } => (name, quantity, category_id),
        _ => (String::new(), String::new(), None),
    };

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
    })
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
