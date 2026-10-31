//! Named shopping lists — create, open to continue, rename, delete (MB-0007).
//!
//! Thin handlers: parse → call `ShoppingListRepo` → render. The index's
//! mutations answer htmx with the re-rendered `#lists` fragment; plain
//! requests get a 303 (PRG), or the full page when the input was rejected.
//! All endpoints require an authenticated user, and another user's list is a
//! 404.
//!
//! An open list is also where it is assembled (MB-0004): free-text items,
//! favorites, a category per item, and soft removal with restore. Those
//! mutations answer htmx with the re-rendered `#list-{id}` fragment, and plain
//! requests with a 303 back to the list. A favorite's item copies its name and
//! category at add-time (RR-0001); a free-text item stays uncategorized until
//! the user picks a category. Adding a recipe (MB-0005) adds one item per
//! ingredient the same way, with an optional quantity multiplier.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{Category, ListItem, ProductGroup, ShoppingList};
use kauppa_core::repo::{CategoryRepo, ProductRepo, RecipeRepo, ShoppingListRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::{WebError, WebResult};
use crate::routes::products::{Picker, parse_category};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct NameForm {
    name: String,
}

#[derive(Debug, Deserialize)]
struct ItemForm {
    text: String,
    #[serde(default)]
    quantity: String,
}

#[derive(Debug, Deserialize)]
struct FavoriteForm {
    // Missing and empty both mean "nothing chosen", which becomes a message.
    #[serde(default)]
    product_id: String,
    #[serde(default)]
    quantity: String,
}

#[derive(Debug, Deserialize)]
struct RecipeForm {
    // Missing and empty both mean "nothing chosen", which becomes a message.
    #[serde(default)]
    recipe_id: String,
    #[serde(default)]
    multiplier: String,
}

#[derive(Debug, Deserialize)]
struct CategoryForm {
    #[serde(default)]
    category_id: String,
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
/// full page.
fn wants_fragment(req: &HttpRequest) -> bool {
    is_hx(req) && req.headers().get("HX-Boosted").is_none()
}

#[derive(Debug, Serialize)]
struct IndexView {
    lists: Vec<ListCard>,
    draft_name: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListCard {
    id: Uuid,
    name: String,
    /// Last update, as a day (`31.10.2026`).
    updated: String,
}

#[derive(Debug, Serialize)]
struct ShowView {
    list: ListView,
}

/// An open list and its builder (`lists/_list`).
#[derive(Debug, Serialize)]
struct ListView {
    id: Uuid,
    name: String,
    updated: String,
    items: Vec<ItemRow>,
    removed: Vec<ItemRow>,
    removed_count: usize,
    favorites: Vec<FavoriteGroup>,
    recipes: Vec<RecipeOption>,
    has_categories: bool,
    draft: ItemDraft,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ItemRow {
    id: Uuid,
    text: String,
    quantity: String,
    category: Option<String>,
    picker: Picker,
}

/// Favorites for the "add a favorite" `<select>`, one `<optgroup>` per
/// category.
#[derive(Debug, Serialize)]
struct FavoriteGroup {
    name: Option<String>,
    products: Vec<FavoriteOption>,
}

#[derive(Debug, Serialize)]
struct FavoriteOption {
    id: Uuid,
    name: String,
}

#[derive(Debug, Serialize)]
struct RecipeOption {
    id: Uuid,
    title: String,
}

/// The add forms' values after a rejected submission.
#[derive(Debug, Default, Serialize)]
struct ItemDraft {
    text: String,
    quantity: String,
    /// The recipe multiplier as typed; the form shows 1 when empty.
    multiplier: String,
}

fn item_row(item: ListItem, categories: &[Category]) -> ItemRow {
    let category_id = item.category.as_ref().map(|c| c.id);
    ItemRow {
        id: item.id,
        text: item.text,
        quantity: item.quantity.unwrap_or_default(),
        category: item.category.map(|c| c.name),
        picker: Picker::new(categories, category_id),
    }
}

fn favorite_groups(groups: Vec<ProductGroup>) -> Vec<FavoriteGroup> {
    groups
        .into_iter()
        .map(|g| FavoriteGroup {
            name: g.category.map(|c| c.name),
            products: g
                .products
                .into_iter()
                .map(|p| FavoriteOption {
                    id: p.id,
                    name: p.name,
                })
                .collect(),
        })
        .collect()
}

/// Render an open list as the full page or, for htmx, its `#list-{id}`
/// fragment.
async fn render_list(
    state: &AppState,
    user: Uuid,
    list_id: Uuid,
    fragment: bool,
    status: StatusCode,
    draft: ItemDraft,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let list = ShoppingListRepo::get(&state.pool, user, list_id).await?;
    let items = ShoppingListRepo::items(&state.pool, user, list_id, false).await?;
    let removed = ShoppingListRepo::items(&state.pool, user, list_id, true).await?;
    let categories = CategoryRepo::list_for_owner(&state.pool, user).await?;
    let favorites = ProductRepo::list_for_owner(&state.pool, user).await?;
    let recipes = RecipeRepo::list_for_owner(&state.pool, user).await?;
    let card = ListCard::from(list);
    let removed_count = removed.len();
    let view = ListView {
        id: card.id,
        name: card.name,
        updated: card.updated,
        items: items
            .into_iter()
            .map(|i| item_row(i, &categories))
            .collect(),
        removed: removed
            .into_iter()
            .map(|i| item_row(i, &categories))
            .collect(),
        removed_count,
        favorites: favorite_groups(favorites),
        recipes: recipes
            .into_iter()
            .map(|r| RecipeOption {
                id: r.id,
                title: r.title,
            })
            .collect(),
        has_categories: !categories.is_empty(),
        draft,
        error,
    };
    let body = if fragment {
        state.templates.render("lists/_list", &view)?
    } else {
        state
            .templates
            .render("lists/show", &ShowView { list: view })?
    };
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// Answer an item mutation on an open list, like `respond` does for the
/// index: the fragment for htmx (rejections included, with 200), else a 303
/// back to the list, or the full page with 400 for rejected input.
async fn respond_item<T>(
    state: &AppState,
    req: &HttpRequest,
    user: Uuid,
    list_id: Uuid,
    outcome: CoreResult<T>,
    draft: ItemDraft,
) -> WebResult<HttpResponse> {
    let error = match outcome {
        Ok(_) => None,
        Err(CoreError::Invalid(m)) => Some(m),
        Err(e) => return Err(e.into()),
    };
    if let Some(reason) = &error {
        tracing::debug!(user_id = %user, list_id = %list_id, %reason, "list item rejected");
    }

    if is_hx(req) {
        render_list(state, user, list_id, true, StatusCode::OK, draft, error).await
    } else if error.is_none() {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, format!("/lists/{list_id}")))
            .finish())
    } else {
        let status = StatusCode::BAD_REQUEST;
        render_list(state, user, list_id, false, status, draft, error).await
    }
}

fn parse_recipe(raw: &str) -> CoreResult<Uuid> {
    match raw.trim() {
        "" => Err(CoreError::Invalid("choose a recipe to add".to_owned())),
        raw => Uuid::parse_str(raw).map_err(|_| CoreError::Invalid("unknown recipe".to_owned())),
    }
}

/// The recipe multiplier: blank means 1, and a decimal comma is accepted
/// ("1,5"). Its range is checked by the repo.
fn parse_multiplier(raw: &str) -> CoreResult<f64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(1.0);
    }
    raw.replace(',', ".").parse::<f64>().map_err(|_| {
        CoreError::Invalid("the multiplier must be a number, e.g. 2 or 1,5".to_owned())
    })
}

fn parse_product(raw: &str) -> CoreResult<Uuid> {
    match raw.trim() {
        "" => Err(CoreError::Invalid("choose a favorite to add".to_owned())),
        raw => Uuid::parse_str(raw).map_err(|_| CoreError::Invalid("unknown product".to_owned())),
    }
}

impl From<ShoppingList> for ListCard {
    fn from(list: ShoppingList) -> Self {
        Self {
            id: list.id,
            updated: list.updated_at.format("%-d.%-m.%Y").to_string(),
            name: list.name,
        }
    }
}

/// Render the index as the full page or, for htmx, the `#lists` fragment.
async fn render_index(
    state: &AppState,
    user: Uuid,
    fragment: bool,
    status: StatusCode,
    draft_name: String,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let lists = ShoppingListRepo::list_for_owner(&state.pool, user).await?;
    let view = IndexView {
        lists: lists.into_iter().map(ListCard::from).collect(),
        draft_name,
        error,
    };
    let template = if fragment {
        "lists/_index"
    } else {
        "lists/index"
    };
    let body = state.templates.render(template, &view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// Answer an index mutation. Rejected input (`Invalid`) is shown to the user:
/// in the fragment for htmx — with 200, since htmx doesn't swap 4xx responses
/// by default — or on the full page with 400. Success is the fragment for
/// htmx, else a 303 to `location`. `NotFound` (another user's list) is a 404.
async fn respond<T>(
    state: &AppState,
    req: &HttpRequest,
    user: Uuid,
    outcome: CoreResult<T>,
    location: impl FnOnce(&T) -> String,
    draft_name: String,
) -> WebResult<HttpResponse> {
    let outcome = match outcome {
        Ok(value) => Ok(value),
        Err(CoreError::Invalid(m)) => Err(m),
        Err(e) => return Err(e.into()),
    };
    if let Err(reason) = &outcome {
        tracing::debug!(user_id = %user, %reason, "list input rejected");
    }

    match outcome {
        _ if is_hx(req) => {
            let error = outcome.err();
            render_index(state, user, true, StatusCode::OK, draft_name, error).await
        }
        Ok(value) => Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, location(&value)))
            .finish()),
        Err(error) => {
            let status = StatusCode::BAD_REQUEST;
            render_index(state, user, false, status, draft_name, Some(error)).await
        }
    }
}

#[get("")]
pub async fn index(state: web::Data<AppState>, id: Identity) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    render_index(&state, user, false, StatusCode::OK, String::new(), None).await
}

/// Create a list. Without htmx the user lands in the new list.
#[post("")]
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    form: web::Form<NameForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ShoppingListRepo::create(&state.pool, user, &form.name).await;
    let draft = if outcome.is_ok() {
        String::new()
    } else {
        form.name.clone()
    };
    respond(
        &state,
        &req,
        user,
        outcome,
        |list| format!("/lists/{}", list.id),
        draft,
    )
    .await
}

/// Open a list to continue it.
#[get("/{id}")]
pub async fn show(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let fragment = wants_fragment(&req);
    let draft = ItemDraft::default();
    render_list(
        &state,
        user,
        path.into_inner(),
        fragment,
        StatusCode::OK,
        draft,
        None,
    )
    .await
}

#[post("/{id}/rename")]
pub async fn rename(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<NameForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ShoppingListRepo::rename(&state.pool, user, path.into_inner(), &form.name).await;
    respond(
        &state,
        &req,
        user,
        outcome,
        |_| "/lists".to_owned(),
        String::new(),
    )
    .await
}

#[post("/{id}/delete")]
pub async fn delete(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ShoppingListRepo::delete(&state.pool, user, path.into_inner()).await;
    respond(
        &state,
        &req,
        user,
        outcome,
        |()| "/lists".to_owned(),
        String::new(),
    )
    .await
}

/// Add a free-text item; it stays uncategorized until a category is chosen.
#[post("/{id}/items")]
pub async fn add_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<ItemForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let outcome =
        ShoppingListRepo::add_item(&state.pool, user, list_id, &form.text, Some(&form.quantity))
            .await;
    let draft = if outcome.is_ok() {
        ItemDraft::default()
    } else {
        let form = form.into_inner();
        ItemDraft {
            text: form.text,
            quantity: form.quantity,
            ..ItemDraft::default()
        }
    };
    respond_item(&state, &req, user, list_id, outcome, draft).await
}

/// Add one of the user's favorites, copying its name and category now.
#[post("/{id}/favorites")]
pub async fn add_favorite(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<FavoriteForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let outcome = match parse_product(&form.product_id) {
        Ok(product) => {
            ShoppingListRepo::add_product(&state.pool, user, list_id, product, Some(&form.quantity))
                .await
        }
        Err(e) => Err(e),
    };
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

/// Add a recipe's ingredients, optionally multiplied (MB-0005).
#[post("/{id}/recipes")]
pub async fn add_recipe(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<RecipeForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let list_id = path.into_inner();
    let outcome = match (
        parse_recipe(&form.recipe_id),
        parse_multiplier(&form.multiplier),
    ) {
        (Ok(recipe), Ok(multiplier)) => {
            ShoppingListRepo::add_recipe(&state.pool, user, list_id, recipe, multiplier).await
        }
        (Err(e), _) | (_, Err(e)) => Err(e),
    };
    let draft = ItemDraft {
        multiplier: if outcome.is_ok() {
            String::new()
        } else {
            form.into_inner().multiplier
        },
        ..ItemDraft::default()
    };
    respond_item(&state, &req, user, list_id, outcome, draft).await
}

#[post("/{id}/items/{item}/category")]
pub async fn assign_category(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
    form: web::Form<CategoryForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = match parse_category(&form.category_id) {
        Ok(category) => {
            ShoppingListRepo::assign_category(&state.pool, user, list_id, item_id, category).await
        }
        Err(e) => Err(e),
    };
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

/// Remove an item (soft delete; it can be restored).
#[post("/{id}/items/{item}/remove")]
pub async fn remove_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = ShoppingListRepo::remove_item(&state.pool, user, list_id, item_id).await;
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

#[post("/{id}/items/{item}/restore")]
pub async fn restore_item(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<(Uuid, Uuid)>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let (list_id, item_id) = path.into_inner();
    let outcome = ShoppingListRepo::restore_item(&state.pool, user, list_id, item_id).await;
    respond_item(&state, &req, user, list_id, outcome, ItemDraft::default()).await
}

// MB-0007 acceptance tests (P3, P5). They sit at module level rather than in a
// `tests` submodule so the brief's `cargo test -p kauppa-web
// routes::lists::<name>` checks select them.

/// List names in index order.
#[cfg(test)]
#[allow(clippy::expect_used)]
async fn list_names(state: &AppState, user: Uuid) -> Vec<String> {
    ShoppingListRepo::list_for_owner(&state.pool, user)
        .await
        .expect("list lists")
        .into_iter()
        .map(|l| l.name)
        .collect()
}

/// P3: opening a list by id loads it to continue — as a page, after boosted
/// navigation, and as a fragment — while another user's list, or no session,
/// is rejected.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn open_resume_owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    make_user(&state, "other@example.com").await;
    let list = ShoppingListRepo::create(&state.pool, user, "Viikonloppu")
        .await
        .expect("create list");
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let open = || TestRequest::get().uri(&format!("/lists/{}", list.id));

    // The index links to it, and opening it shows it.
    let my_index = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/lists")
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(my_index.contains(&format!(r#"href="/lists/{}""#, list.id)));
    let resp = call_service(&app, open().cookie(session.clone()).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("Viikonloppu"));
    assert!(body.contains(&format!(r#"data-list-id="{}""#, list.id)));

    // Boosted navigation gets the page; a plain htmx request the fragment.
    let boosted = body_string(
        call_service(
            &app,
            open()
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .insert_header(("HX-Boosted", "true"))
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(boosted.contains("<html"));
    let fragment = body_string(
        call_service(
            &app,
            open()
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(!fragment.contains("<html"));
    assert!(fragment.contains(&format!(r#"id="list-{}""#, list.id)));

    // Another user: 404, for the page and the fragment; never on their index.
    for hx in [false, true] {
        let mut req = open().cookie(intruder.clone());
        if hx {
            req = req.insert_header(("HX-Request", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "htmx: {hx}");
    }
    let their_index = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/lists")
                .cookie(intruder.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(!their_index.contains(&list.id.to_string()));

    // A missing list is a 404 too; no session is a 401.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/lists/{}", Uuid::new_v4()))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let resp = call_service(&app, open().to_request()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

/// P5: create, rename, and delete answer htmx with the `#lists` fragment and
/// plain requests with a 303 (or the full page when the input was rejected);
/// another user's list is a 404 and stays unchanged; every endpoint rejects a
/// request without a session.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn lifecycle_htmx_and_auth() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    make_user(&state, "other@example.com").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);
    let post = |uri: String, name: &str| {
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form([("name", name.to_owned())])
    };
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true"));

    // Create over htmx: the fragment with the new list.
    let resp = call_service(&app, hx(post("/lists".to_owned(), "Arki")).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(r#"id="lists""#));
    assert!(body.contains("Arki"));

    // Create without htmx: a 303 into the new list.
    let resp = call_service(&app, post("/lists".to_owned(), "Juhlat").to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .expect("location")
        .to_str()
        .expect("ascii")
        .to_owned();
    let juhlat: Uuid = location
        .strip_prefix("/lists/")
        .expect("list url")
        .parse()
        .expect("list id");
    assert_eq!(list_names(&state, user).await, ["Juhlat", "Arki"]);

    // A rejected name: in the fragment over htmx, the full page with 400
    // otherwise; nothing is created.
    let resp = call_service(&app, hx(post("/lists".to_owned(), "  ")).to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("must not be empty"));
    let resp = call_service(&app, post("/lists".to_owned(), "").to_request()).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("must not be empty"));
    assert_eq!(list_names(&state, user).await.len(), 2);

    // Rename: fragment over htmx, 303 otherwise; it moves to the top.
    let arki = ShoppingListRepo::list_for_owner(&state.pool, user)
        .await
        .expect("lists")
        .into_iter()
        .find(|l| l.name == "Arki")
        .expect("Arki")
        .id;
    let resp = call_service(
        &app,
        hx(post(format!("/lists/{arki}/rename"), "Arkiruoka")).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Arkiruoka"));
    let resp = call_service(
        &app,
        post(format!("/lists/{juhlat}/rename"), "Synttärit").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(list_names(&state, user).await, ["Synttärit", "Arkiruoka"]);

    // Another user can't rename or delete; nothing changes.
    for uri in [
        format!("/lists/{arki}/rename"),
        format!("/lists/{arki}/delete"),
    ] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&uri)
                .cookie(intruder.clone())
                .set_form([("name", "Kaapattu")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri}");
    }
    assert_eq!(list_names(&state, user).await, ["Synttärit", "Arkiruoka"]);

    // Delete: fragment over htmx, 303 otherwise.
    let resp = call_service(
        &app,
        hx(TestRequest::post()
            .uri(&format!("/lists/{arki}/delete"))
            .cookie(session.clone()))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(r#"id="lists""#) && !body.contains("Arkiruoka"));
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{juhlat}/delete"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(list_names(&state, user).await, Vec::<String>::new());

    // No session: every endpoint is rejected.
    let id = Uuid::new_v4();
    for req in [
        TestRequest::get().uri("/lists"),
        TestRequest::post().uri("/lists").set_form([("name", "x")]),
        TestRequest::get().uri(&format!("/lists/{id}")),
        TestRequest::post()
            .uri(&format!("/lists/{id}/rename"))
            .set_form([("name", "x")]),
        TestRequest::post().uri(&format!("/lists/{id}/delete")),
    ] {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}

// MB-0004 acceptance tests (P5, P6), at module level for the brief's
// `cargo test -p kauppa-web routes::lists::<name>` checks.

/// `(text, category)` of a list's active items, in order.
#[cfg(test)]
#[allow(clippy::expect_used)]
async fn item_shape(state: &AppState, user: Uuid, list: Uuid) -> Vec<(String, Option<String>)> {
    ShoppingListRepo::items(&state.pool, user, list, false)
        .await
        .expect("list items")
        .into_iter()
        .map(|i| (i.text, i.category.map(|c| c.name)))
        .collect()
}

/// MB-0004 P5: items are isolated through their list — another user can't
/// read the list or add, categorize, remove, or restore its items, over htmx
/// or not, and nothing changes.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let their_category = make_category(&state, other, "Kylmä").await;
    let their_milk = ProductRepo::create(&state.pool, other, "Maito", their_category)
        .await
        .expect("their favorite");
    let list = ShoppingListRepo::create(&state.pool, user, "Oma")
        .await
        .expect("list")
        .id;
    let bread = ShoppingListRepo::add_item(&state.pool, user, list, "Leipä", None)
        .await
        .expect("item")
        .id;
    let gone = ShoppingListRepo::add_item(&state.pool, user, list, "Sokeri", None)
        .await
        .expect("item")
        .id;
    ShoppingListRepo::remove_item(&state.pool, user, list, gone)
        .await
        .expect("remove");
    let intruder =
        session_cookie(&call_service(&app, login_request("other@example.com").to_request()).await);

    let base = format!("/lists/{list}");
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&base)
            .cookie(intruder.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let writes: [(String, Vec<(&str, String)>); 5] = [
        (format!("{base}/items"), vec![("text", "Omena".to_owned())]),
        (
            format!("{base}/favorites"),
            vec![("product_id", their_milk.id.to_string())],
        ),
        (
            format!("{base}/items/{bread}/category"),
            vec![("category_id", kylma.to_string())],
        ),
        (format!("{base}/items/{bread}/remove"), vec![]),
        (format!("{base}/items/{gone}/restore"), vec![]),
    ];
    for (uri, form) in writes {
        for hx in [false, true] {
            let mut req = TestRequest::post()
                .uri(&uri)
                .cookie(intruder.clone())
                .set_form(form.clone());
            if hx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri} (htmx: {hx})");
        }
    }

    assert_eq!(
        item_shape(&state, user, list).await,
        [("Leipä".to_owned(), None)]
    );
    let removed = ShoppingListRepo::items(&state.pool, user, list, true)
        .await
        .expect("removed");
    assert_eq!(removed.len(), 1, "still removed");

    // And the owner can't add another user's favorite to their own list.
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{base}/favorites"))
            .cookie(session)
            .set_form([("product_id", their_milk.id.to_string())])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(item_shape(&state, user, list).await.len(), 1);
}

/// MB-0004 P6: add-item, add-favorite, assign-category, and remove (and
/// restore) answer htmx with the `#list-{id}` fragment and plain requests with
/// a 303, or the full page with 400 when the input was rejected.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn htmx_fragments() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let koti = make_category(&state, user, "Koti").await;
    let maito = ProductRepo::create(&state.pool, user, "Maito", kylma)
        .await
        .expect("favorite");
    let list = ShoppingListRepo::create(&state.pool, user, "Viikko")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let base = format!("/lists/{list}");
    let post = |uri: String, form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let fragment_id = format!(r#"id="list-{list}""#);

    // htmx: each mutation answers with the fragment.
    let hx_call = |req: TestRequest| {
        let app = &app;
        async move {
            let resp =
                call_service(app, req.insert_header(("HX-Request", "true")).to_request()).await;
            assert_eq!(resp.status(), StatusCode::OK);
            body_string(resp).await
        }
    };
    let body = hx_call(post(
        format!("{base}/items"),
        vec![
            ("text", "Kynttilät".to_owned()),
            ("quantity", "6".to_owned()),
        ],
    ))
    .await;
    assert!(!body.contains("<html") && body.contains(&fragment_id));
    assert!(body.contains("Kynttilät") && body.contains("Uncategorized"));
    let candles = ShoppingListRepo::items(&state.pool, user, list, false)
        .await
        .expect("items")[0]
        .id;

    let body = hx_call(post(
        format!("{base}/favorites"),
        vec![("product_id", maito.id.to_string())],
    ))
    .await;
    assert!(body.contains(&fragment_id) && body.contains("Maito"));

    let body = hx_call(post(
        format!("{base}/items/{candles}/category"),
        vec![("category_id", koti.to_string())],
    ))
    .await;
    assert!(body.contains(&fragment_id) && body.contains("· Koti"));

    let body = hx_call(post(format!("{base}/items/{candles}/remove"), vec![])).await;
    assert!(body.contains(&fragment_id) && body.contains("Removed (1)"));
    assert_eq!(
        item_shape(&state, user, list).await,
        [("Maito".to_owned(), Some("Kylmä".to_owned()))]
    );

    let body = hx_call(post(format!("{base}/items/{candles}/restore"), vec![])).await;
    assert!(body.contains(&fragment_id) && !body.contains("Removed ("));

    // htmx rejections stay in the fragment, keeping the typed values.
    let body = hx_call(post(
        format!("{base}/items"),
        vec![("text", " ".to_owned()), ("quantity", "2".to_owned())],
    ))
    .await;
    assert!(body.contains(&fragment_id) && body.contains("must not be empty"));
    assert!(body.contains(r#"value="2""#));
    let body = hx_call(post(format!("{base}/favorites"), vec![])).await;
    assert!(body.contains("choose a favorite"));

    // Without htmx: a 303 back to the list on success.
    for (uri, form) in [
        (format!("{base}/items"), vec![("text", "Leipä".to_owned())]),
        (
            format!("{base}/favorites"),
            vec![
                ("product_id", maito.id.to_string()),
                ("quantity", "1 l".to_owned()),
            ],
        ),
        (
            format!("{base}/items/{candles}/category"),
            vec![("category_id", kylma.to_string())],
        ),
        (format!("{base}/items/{candles}/remove"), vec![]),
        (format!("{base}/items/{candles}/restore"), vec![]),
    ] {
        let resp = call_service(&app, post(uri.clone(), form).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{uri}");
        assert_eq!(
            resp.headers().get(header::LOCATION).expect("location"),
            base.as_str()
        );
    }
    assert_eq!(
        item_shape(&state, user, list).await,
        [
            ("Kynttilät".to_owned(), Some("Kylmä".to_owned())),
            ("Maito".to_owned(), Some("Kylmä".to_owned())),
            ("Leipä".to_owned(), None),
            ("Maito".to_owned(), Some("Kylmä".to_owned())),
        ]
    );

    // Without htmx, rejected input is the full page with 400.
    for (uri, form, message) in [
        (
            format!("{base}/items"),
            vec![("text", String::new())],
            "must not be empty",
        ),
        (
            format!("{base}/favorites"),
            vec![("product_id", "nope".to_owned())],
            "unknown product",
        ),
        (
            format!("{base}/items/{candles}/category"),
            vec![("category_id", String::new())],
            "choose a category",
        ),
    ] {
        let resp = call_service(&app, post(uri.clone(), form).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{uri}");
        let body = body_string(resp).await;
        assert!(body.contains("<html") && body.contains(message), "{uri}");
    }

    // The page shows the builder with the favorites picker.
    let page = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&base)
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(page.contains("<html") && page.contains(&fragment_id));
    assert!(page.contains(&format!(r#"<option value="{}">Maito</option>"#, maito.id)));
}

// MB-0005 acceptance tests (P5, P6), at module level for the brief's
// `cargo test -p kauppa-web routes::lists::<name>` checks.

/// A recipe with one ingredient per `(product name, quantity)`, all in
/// `category`.
#[cfg(test)]
#[allow(clippy::expect_used)]
async fn make_recipe(
    state: &AppState,
    owner: Uuid,
    title: &str,
    category: Uuid,
    ingredients: &[(&str, Option<&str>)],
) -> Uuid {
    use kauppa_core::domain::{NewIngredient, RecipeInput};

    let recipe = RecipeRepo::create(
        &state.pool,
        owner,
        &RecipeInput {
            title: title.to_owned(),
            ..RecipeInput::default()
        },
    )
    .await
    .expect("create recipe")
    .id;
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

/// MB-0005 P5: a user can't add from another user's recipe, nor onto another
/// user's list — over htmx or not — and nothing is added anywhere.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_recipe_owner_isolation() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let mine = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "Kylmä").await;
    let my_recipe = make_recipe(&state, user, "Puuro", mine, &[("Maito", Some("5 dl"))]).await;
    let their_recipe = make_recipe(
        &state,
        other,
        "Salaatti",
        theirs,
        &[("Feta", Some("1 pkt"))],
    )
    .await;
    let my_list = ShoppingListRepo::create(&state.pool, user, "Oma")
        .await
        .expect("list")
        .id;
    let their_list = ShoppingListRepo::create(&state.pool, other, "Heidän")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    for (list, recipe) in [
        (my_list, their_recipe),
        (their_list, my_recipe),
        (their_list, their_recipe),
        (my_list, Uuid::new_v4()),
    ] {
        for hx in [false, true] {
            let mut req = TestRequest::post()
                .uri(&format!("/lists/{list}/recipes"))
                .cookie(session.clone())
                .set_form([
                    ("recipe_id", recipe.to_string()),
                    ("multiplier", "2".to_owned()),
                ]);
            if hx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            assert_eq!(
                resp.status(),
                StatusCode::NOT_FOUND,
                "{list}/{recipe} (htmx: {hx})"
            );
        }
    }
    assert_eq!(item_shape(&state, user, my_list).await.len(), 0);
    assert_eq!(item_shape(&state, other, their_list).await.len(), 0);

    // The list page offers only the user's own recipes.
    let page = crate::routes::test_support::body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri(&format!("/lists/{my_list}"))
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    assert!(page.contains(&my_recipe.to_string()));
    assert!(!page.contains(&their_recipe.to_string()));
}

/// MB-0005 P6: adding a recipe answers htmx with the `#list-{id}` fragment
/// and plain requests with a 303 (or the full page with 400 for rejected
/// input); without a session it is rejected.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn add_recipe_htmx() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let pancakes = make_recipe(
        &state,
        user,
        "Pannukakku",
        kylma,
        &[("Maito", Some("5 dl")), ("Kananmuna", Some("2"))],
    )
    .await;
    let empty = make_recipe(&state, user, "Tyhjä", kylma, &[]).await;
    let list = ShoppingListRepo::create(&state.pool, user, "Viikko")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let uri = format!("/lists/{list}/recipes");
    let post = |form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri(&uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let fragment_id = format!(r#"id="list-{list}""#);

    // htmx: the fragment with the scaled ingredients.
    let resp = call_service(
        &app,
        post(vec![
            ("recipe_id", pancakes.to_string()),
            ("multiplier", "1,5".to_owned()),
        ])
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html") && body.contains(&fragment_id));
    assert!(body.contains("7.5 dl") && body.contains("Maito") && body.contains("· Kylmä"));

    // Plain: a 303 back to the list; a blank multiplier means 1.
    let resp = call_service(
        &app,
        post(vec![
            ("recipe_id", pancakes.to_string()),
            ("multiplier", String::new()),
        ])
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers().get(header::LOCATION).expect("location"),
        format!("/lists/{list}").as_str()
    );
    assert_eq!(
        ShoppingListRepo::items(&state.pool, user, list, false)
            .await
            .expect("items")
            .into_iter()
            .map(|i| i.quantity.unwrap_or_default())
            .collect::<Vec<_>>(),
        ["7.5 dl", "3", "5 dl", "2"]
    );

    // Rejected input: in the fragment over htmx (keeping the multiplier),
    // the full page with 400 otherwise; nothing is added.
    let rejected = [
        (vec![("multiplier", "2".to_owned())], "choose a recipe"),
        (vec![("recipe_id", "nope".to_owned())], "unknown recipe"),
        (
            vec![
                ("recipe_id", pancakes.to_string()),
                ("multiplier", "kaksi".to_owned()),
            ],
            "must be a number",
        ),
        (
            vec![
                ("recipe_id", pancakes.to_string()),
                ("multiplier", "0".to_owned()),
            ],
            "more than 0",
        ),
        (vec![("recipe_id", empty.to_string())], "has no ingredients"),
    ];
    for (form, message) in rejected {
        let resp = call_service(
            &app,
            post(form.clone())
                .insert_header(("HX-Request", "true"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "{form:?}");
        let body = body_string(resp).await;
        assert!(
            body.contains(&fragment_id) && body.contains(message),
            "{form:?}"
        );
        let resp = call_service(&app, post(form.clone()).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{form:?}");
        let body = body_string(resp).await;
        assert!(body.contains("<html") && body.contains(message), "{form:?}");
    }
    let body = body_string(
        call_service(
            &app,
            post(vec![
                ("recipe_id", pancakes.to_string()),
                ("multiplier", "kaksi".to_owned()),
            ])
            .insert_header(("HX-Request", "true"))
            .to_request(),
        )
        .await,
    )
    .await;
    assert!(
        body.contains(r#"value="kaksi""#),
        "keeps the typed multiplier"
    );
    assert_eq!(item_shape(&state, user, list).await.len(), 4);

    // Signed out: rejected.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&uri)
            .set_form([("recipe_id", pancakes.to_string())])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
