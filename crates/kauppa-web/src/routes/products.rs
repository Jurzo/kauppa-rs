//! Favorite products — the per-user product catalog (MB-0002) — and, below
//! them, the products created from recipe ingredients (MB-0014).
//!
//! Thin handlers: parse → call `ProductRepo` → render. htmx requests get the
//! re-rendered catalog fragment; plain requests get a 303 back to the page
//! (PRG), or the full page when the input was rejected. All endpoints require
//! an authenticated user.
//!
//! RR-0001: a favorite must have a category. The column is nullable (deleting
//! a category un-categorizes its products), so the rule lives here, in
//! `parse_category`; the repo additionally refuses a category the user doesn't
//! own.
//!
//! The rename, category, and delete handlers work on any product the user
//! owns, favorite or recipe-created; `make_favorite` promotes a recipe-created
//! one in place.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{Category, Direction, HiddenProductGroup, Product, ProductGroup};
use kauppa_core::repo::{CategoryRepo, ProductRepo};
use kauppa_core::{CoreError, CoreResult};

use crate::error::{WebError, WebResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct CreateForm {
    name: String,
    // A missing field and an empty one both mean "no category chosen", which
    // `parse_category` turns into a user-facing message rather than a 400
    // from the extractor.
    #[serde(default)]
    category_id: String,
}

#[derive(Debug, Deserialize)]
struct NameForm {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CategoryForm {
    #[serde(default)]
    category_id: String,
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

/// Enforce "a favorite must have a category" (RR-0001) on the submitted id.
/// Whether the category is the user's is checked by the repo.
pub fn parse_category(raw: &str) -> CoreResult<Uuid> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CoreError::Invalid(
            "choose a category for the product".to_owned(),
        ));
    }
    Uuid::parse_str(raw).map_err(|_| CoreError::Invalid("unknown category".to_owned()))
}

/// What the "add" form shows after a response: the category is kept after an
/// add (quick entry of several products), the name only after a rejected one.
#[derive(Debug, Default)]
struct Draft {
    name: String,
    category_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct CatalogView {
    has_categories: bool,
    add: AddView,
    groups: Vec<GroupView>,
    /// Recipe-created products (MB-0014), shown below the favorites.
    has_hidden: bool,
    hidden: Vec<HiddenGroupView>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AddView {
    name: String,
    picker: Picker,
}

#[derive(Debug, Serialize)]
struct GroupView {
    /// `None` for the uncategorized group.
    name: Option<String>,
    products: Vec<ProductView>,
}

/// A product row's edit controls (`products/_product_edit`).
#[derive(Debug, Serialize)]
struct ProductView {
    id: Uuid,
    name: String,
    picker: Picker,
    /// The delete button's `hx-confirm` text.
    delete_confirm: String,
    /// Shown next to the delete button, for products recipes use.
    delete_warning: Option<String>,
}

impl ProductView {
    /// `recipe_count` is how many of the user's recipes use the product; the
    /// delete confirmation names it (MB-0014 P5).
    fn new(categories: &[Category], product: Product, recipe_count: i64) -> Self {
        let mut delete_confirm = format!("Delete “{}”?", product.name);
        let delete_warning = (recipe_count > 0).then(|| {
            format!(
                "Used in {}. Deleting it keeps the ingredient in those recipes, \
                 by name and quantity, no longer linked to a product.",
                recipes(recipe_count)
            )
        });
        if let Some(warning) = &delete_warning {
            delete_confirm.push(' ');
            delete_confirm.push_str(warning);
        }
        Self {
            picker: Picker::new(categories, product.category_id),
            id: product.id,
            name: product.name,
            delete_confirm,
            delete_warning,
        }
    }
}

/// "1 recipe", "2 recipes".
fn recipes(count: i64) -> String {
    if count == 1 {
        "1 recipe".to_owned()
    } else {
        format!("{count} recipes")
    }
}

#[derive(Debug, Serialize)]
struct HiddenGroupView {
    /// `None` for the uncategorized group.
    name: Option<String>,
    products: Vec<HiddenProductView>,
}

/// A recipe-created product: the shared edit controls plus its usage, and
/// whether it can be made a favorite (only with a category, RR-0001).
#[derive(Debug, Serialize)]
struct HiddenProductView {
    #[serde(flatten)]
    product: ProductView,
    usage: String,
    can_favorite: bool,
}

/// A category `<select>` (`products/_category_picker`); `has_selection` is
/// false when the placeholder ("Choose a category") should be selected
/// instead. Also used by the recipe editor, where it is optional.
#[derive(Debug, Serialize)]
pub struct Picker {
    options: Vec<PickerOption>,
    has_selection: bool,
    required: bool,
}

#[derive(Debug, Serialize)]
struct PickerOption {
    id: Uuid,
    name: String,
    selected: bool,
}

impl Picker {
    /// A required picker over `categories`, with `selected` chosen.
    pub fn new(categories: &[Category], selected: Option<Uuid>) -> Self {
        let options: Vec<PickerOption> = categories
            .iter()
            .map(|c| PickerOption {
                id: c.id,
                name: c.name.clone(),
                selected: Some(c.id) == selected,
            })
            .collect();
        let has_selection = options.iter().any(|o| o.selected);
        Self {
            options,
            has_selection,
            required: true,
        }
    }

    /// The same picker, but submittable without a choice.
    pub const fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

impl CatalogView {
    fn new(
        categories: &[Category],
        groups: Vec<ProductGroup>,
        hidden: Vec<HiddenProductGroup>,
        draft: Draft,
        error: Option<String>,
    ) -> Self {
        // Favorites have no usage count here; their delete confirmation stays
        // as it was (MB-0002).
        let groups = groups
            .into_iter()
            .map(|group| GroupView {
                name: group.category.map(|c| c.name),
                products: group
                    .products
                    .into_iter()
                    .map(|p| ProductView::new(categories, p, 0))
                    .collect(),
            })
            .collect();
        let hidden: Vec<HiddenGroupView> = hidden
            .into_iter()
            .map(|group| {
                let can_favorite = group.category.is_some();
                HiddenGroupView {
                    name: group.category.map(|c| c.name),
                    products: group
                        .products
                        .into_iter()
                        .map(|u| HiddenProductView {
                            usage: if u.recipe_count == 0 {
                                "Not used in any recipe".to_owned()
                            } else {
                                format!("Used in {}", recipes(u.recipe_count))
                            },
                            can_favorite,
                            product: ProductView::new(categories, u.product, u.recipe_count),
                        })
                        .collect(),
                }
            })
            .collect();
        Self {
            has_categories: !categories.is_empty(),
            add: AddView {
                name: draft.name,
                picker: Picker::new(categories, draft.category_id),
            },
            groups,
            has_hidden: !hidden.is_empty(),
            hidden,
            error,
        }
    }
}

/// Render the catalog as the full page or, for htmx, the `#favorites` fragment.
async fn render(
    state: &AppState,
    user: Uuid,
    fragment: bool,
    status: StatusCode,
    draft: Draft,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let categories = CategoryRepo::list_for_owner(&state.pool, user).await?;
    let groups = ProductRepo::list_for_owner(&state.pool, user).await?;
    let hidden = ProductRepo::list_hidden_with_usage(&state.pool, user).await?;
    let view = CatalogView::new(&categories, groups, hidden, draft, error);
    let template = if fragment {
        "products/_catalog"
    } else {
        "products/index"
    };
    let body = state.templates.render(template, &view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .body(body))
}

/// Answer a mutation. Rejected input (`Invalid` / `Conflict`) is shown to the
/// user: in the fragment for htmx — with 200, since htmx doesn't swap 4xx
/// responses by default — or on the full page with the 4xx status. Success is
/// the fragment for htmx, else a 303 back to the page.
async fn respond<T>(
    state: &AppState,
    req: &HttpRequest,
    user: Uuid,
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
        tracing::debug!(user_id = %user, %reason, "favorites input rejected");
    }

    if is_hx(req) {
        render(state, user, true, StatusCode::OK, draft, error).await
    } else if error.is_none() {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, "/favorites"))
            .finish())
    } else {
        render(state, user, false, status, draft, error).await
    }
}

#[get("")]
pub async fn index(state: web::Data<AppState>, id: Identity) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    render(&state, user, false, StatusCode::OK, Draft::default(), None).await
}

#[post("")]
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    form: web::Form<CreateForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let category = parse_category(&form.category_id);
    let category_id = category.as_ref().ok().copied();
    let outcome = match category {
        Ok(category) => ProductRepo::create(&state.pool, user, &form.name, category).await,
        Err(e) => Err(e),
    };
    let draft = Draft {
        name: if outcome.is_ok() {
            String::new()
        } else {
            form.name.clone()
        },
        category_id,
    };
    respond(&state, &req, user, outcome, draft).await
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
    let outcome = ProductRepo::rename(&state.pool, user, path.into_inner(), &form.name).await;
    respond(&state, &req, user, outcome, Draft::default()).await
}

#[post("/{id}/category")]
pub async fn recategorize(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<CategoryForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = match parse_category(&form.category_id) {
        Ok(category) => {
            ProductRepo::recategorize(&state.pool, user, path.into_inner(), category).await
        }
        Err(e) => Err(e),
    };
    respond(&state, &req, user, outcome, Draft::default()).await
}

/// Move a product one step up or down within its category.
#[post("/{id}/move")]
pub async fn move_product(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<MoveForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome =
        ProductRepo::move_within_category(&state.pool, user, path.into_inner(), form.dir).await;
    respond(&state, &req, user, outcome, Draft::default()).await
}

/// Make a recipe-created product a favorite, in its own category (MB-0014).
#[post("/{id}/favorite")]
pub async fn make_favorite(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ProductRepo::make_favorite(&state.pool, user, path.into_inner()).await;
    respond(&state, &req, user, outcome, Draft::default()).await
}

#[post("/{id}/delete")]
pub async fn delete(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = ProductRepo::delete(&state.pool, user, path.into_inner()).await;
    respond(&state, &req, user, outcome, Draft::default()).await
}

// MB-0002 acceptance tests (P1, P6). They sit at module level rather than in a
// `tests` submodule so the brief's `cargo test -p kauppa-web
// routes::products::<name>` checks select them.

/// Product names per group, in display order; `None` is uncategorized.
#[cfg(test)]
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

#[cfg(test)]
#[allow(clippy::expect_used)]
async fn product_id(state: &AppState, user: Uuid, name: &str) -> Uuid {
    ProductRepo::list_for_owner(&state.pool, user)
        .await
        .expect("list products")
        .into_iter()
        .flat_map(|g| g.products)
        .find(|p| p.name == name)
        .expect("product exists")
        .id
}

/// P1: the server rejects a favorite with a missing, malformed, or foreign
/// category — even when a client skips the `<select required>` — and creates
/// nothing.
#[cfg(test)]
#[actix_web::test]
async fn create_requires_owned_category() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let theirs = make_category(&state, other, "Pakasteet").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let submit = |form: Vec<(&'static str, String)>| {
        TestRequest::post()
            .uri("/favorites")
            .cookie(session.clone())
            .set_form(form)
    };
    let name = || ("name", "Maito".to_owned());

    let rejected = [
        (vec![name()], "choose a category"),
        (
            vec![name(), ("category_id", String::new())],
            "choose a category",
        ),
        (
            vec![name(), ("category_id", "not-a-uuid".to_owned())],
            "unknown category",
        ),
        (
            vec![name(), ("category_id", theirs.to_string())],
            "unknown category",
        ),
        (
            vec![name(), ("category_id", Uuid::new_v4().to_string())],
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

    // Over htmx the rejection comes back as the fragment, carrying the error
    // and keeping the typed name for correction.
    let resp = call_service(
        &app,
        submit(vec![name(), ("category_id", theirs.to_string())])
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        !body.contains("<html"),
        "htmx response should be a fragment"
    );
    assert!(body.contains(r#"role="alert""#) && body.contains("unknown category"));
    assert!(body.contains(r#"value="Maito""#));

    assert!(
        catalog_shape(&state, user).await.is_empty(),
        "nothing created"
    );
    assert!(
        catalog_shape(&state, other).await.is_empty(),
        "nothing created"
    );

    // An owned category is accepted.
    let resp = call_service(
        &app,
        submit(vec![name(), ("category_id", hevi.to_string())]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        catalog_shape(&state, user).await,
        [(Some("Hevi".to_owned()), vec!["Maito".to_owned()])]
    );
}

/// P6: `GET /favorites` renders the grouped catalog; create / edit / delete
/// return fragments over htmx and redirect to the full page otherwise;
/// unauthenticated requests are rejected.
#[cfg(test)]
#[actix_web::test]
async fn crud_and_auth() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);

    // Unauthenticated: every route is rejected before touching anything.
    let any = Uuid::new_v4();
    let any_id = any.to_string();
    let anonymous = [
        TestRequest::get().uri("/favorites"),
        TestRequest::post()
            .uri("/favorites")
            .set_form([("name", "Maito"), ("category_id", any_id.as_str())]),
        TestRequest::post()
            .uri(&format!("/favorites/{any}/rename"))
            .set_form([("name", "Maito")]),
        TestRequest::post()
            .uri(&format!("/favorites/{any}/category"))
            .set_form([("category_id", any_id.as_str())]),
        TestRequest::post()
            .uri(&format!("/favorites/{any}/move"))
            .set_form([("dir", "up")]),
        TestRequest::post().uri(&format!("/favorites/{any}/delete")),
    ];
    for req in anonymous {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true")).to_request();
    let (hevi, kylma) = (hevi.to_string(), kylma.to_string());
    let (hevi, kylma) = (hevi.as_str(), kylma.as_str());

    // The page: a full document with the add form's category picker.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/favorites")
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("Favorites"));
    assert!(body.contains(&format!(r#"<option value="{kylma}""#)));

    // Create over htmx: the fragment, grouped under the category.
    let resp = call_service(
        &app,
        hx(post(
            "/favorites",
            &[("name", "Omena"), ("category_id", hevi)],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        !body.contains("<html"),
        "htmx response should be a fragment"
    );
    assert!(body.contains(r#"id="favorites""#));
    assert!(body.contains("<h2>Hevi</h2>") && body.contains("Omena"));

    // Create without htmx: 303 back to the page.
    let resp = call_service(
        &app,
        post("/favorites", &[("name", "Maito"), ("category_id", kylma)]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/favorites")
    );
    call_service(
        &app,
        hx(post(
            "/favorites",
            &[("name", "Banaani"), ("category_id", hevi)],
        )),
    )
    .await;

    // A duplicate without htmx: the full page with the error, as 409.
    let resp = call_service(
        &app,
        post("/favorites", &[("name", "Maito"), ("category_id", hevi)]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("product already exists: Maito"));

    let omena = product_id(&state, user, "Omena").await;
    let banaani = product_id(&state, user, "Banaani").await;
    let maito = product_id(&state, user, "Maito").await;

    // Rename, reposition, and recategorize over htmx.
    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{omena}/rename"),
            &[("name", "Vihreä omena")],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Vihreä omena"));

    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{banaani}/move"),
            &[("dir", "up")],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let (moved, other) = (body.find("Banaani"), body.find("Vihreä omena"));
    assert!(
        moved.is_some() && moved < other,
        "Banaani moved above Vihreä omena"
    );

    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{banaani}/category"),
            &[("category_id", kylma)],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        catalog_shape(&state, user).await,
        [
            (Some("Hevi".to_owned()), vec!["Vihreä omena".to_owned()]),
            (
                Some("Kylmä".to_owned()),
                vec!["Maito".to_owned(), "Banaani".to_owned()]
            ),
        ]
    );

    // Edits without htmx redirect back to the page.
    let resp = call_service(
        &app,
        post(&format!("/favorites/{maito}/move"), &[("dir", "down")]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Delete over htmx; the product is gone from the fragment.
    let resp = call_service(&app, hx(post(&format!("/favorites/{maito}/delete"), &[]))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!body_string(resp).await.contains("Maito"));
    assert_eq!(
        catalog_shape(&state, user).await,
        [
            (Some("Hevi".to_owned()), vec!["Vihreä omena".to_owned()]),
            (Some("Kylmä".to_owned()), vec!["Banaani".to_owned()]),
        ]
    );
}

/// MB-0010 P6: adding a favorite whose name differs only by case from an
/// existing one shows the "already exists" error, over htmx and without it,
/// and creates nothing.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn case_duplicate_rejected() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    for name in ["maito", "Äyräs"] {
        ProductRepo::create(&state.pool, user, name, kylma)
            .await
            .expect("create product");
    }
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let hevi = hevi.to_string();
    let post = |name: &str| {
        TestRequest::post()
            .uri("/favorites")
            .cookie(session.clone())
            .set_form([("name", name), ("category_id", hevi.as_str())])
    };

    // Over htmx: the fragment with the alert.
    let resp = call_service(
        &app,
        post("Maito")
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        !body.contains("<html"),
        "htmx response should be a fragment"
    );
    assert!(body.contains(r#"role="alert""#) && body.contains("product already exists"));

    // Without htmx: the full page with 409.
    let resp = call_service(&app, post("ÄYRÄS").to_request()).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("product already exists"));

    assert_eq!(
        catalog_shape(&state, user).await,
        [(
            Some("Kylmä".to_owned()),
            vec!["maito".to_owned(), "Äyräs".to_owned()]
        )]
    );
}

// MB-0014 acceptance tests (P3–P5), at module level like the ones above.

/// A recipe titled `title` with one ingredient per `(name, quantity)`; new
/// products are created in `category`, hidden from favorites (MB-0003).
#[cfg(test)]
#[allow(clippy::expect_used)]
async fn make_recipe_with(
    state: &AppState,
    user: Uuid,
    title: &str,
    ingredients: &[(&str, Option<&str>)],
    category: Uuid,
) -> Uuid {
    use kauppa_core::domain::{NewIngredient, RecipeInput};
    use kauppa_core::repo::RecipeRepo;

    let recipe = RecipeRepo::create(
        &state.pool,
        user,
        &RecipeInput {
            title: title.to_owned(),
            ..RecipeInput::default()
        },
    )
    .await
    .expect("create recipe")
    .id;
    for (name, quantity) in ingredients {
        RecipeRepo::add_ingredient(
            &state.pool,
            user,
            recipe,
            &NewIngredient {
                product_name: (*name).to_owned(),
                quantity: quantity.map(str::to_owned),
                category_id: Some(category),
            },
        )
        .await
        .expect("add ingredient");
    }
    recipe
}

/// `(group name, [product names])` of the user's recipe-created products;
/// `None` is uncategorized.
#[cfg(test)]
#[allow(clippy::expect_used)]
async fn hidden_shape(state: &AppState, user: Uuid) -> Vec<(Option<String>, Vec<String>)> {
    ProductRepo::list_hidden_with_usage(&state.pool, user)
        .await
        .expect("list hidden products")
        .into_iter()
        .map(|g| {
            (
                g.category.map(|c| c.name),
                g.products.into_iter().map(|u| u.product.name).collect(),
            )
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
async fn any_product_id(state: &AppState, user: Uuid, name: &str) -> Uuid {
    ProductRepo::find_by_name(&state.pool, user, name)
        .await
        .expect("find product")
        .expect("product exists")
        .id
}

/// The part of a `/favorites` body from the recipe-created section on.
#[cfg(test)]
fn recipe_section(body: &str) -> &str {
    body.find(r#"id="recipe-products""#)
        .map_or("", |start| &body[start..])
}

/// P3: `/favorites` lists the user's recipe-created products below the
/// favorites, grouped by category with their usage counts, and make favorite,
/// rename, change category, and delete work on them over htmx (the fragment)
/// and without it (303). Another user's products never appear and can't be
/// changed.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used, clippy::too_many_lines)]
async fn hidden_products_section() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);

    // Unauthenticated: the new route is rejected like the others.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/favorites/{}/favorite", Uuid::new_v4()))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let kylma = make_category(&state, user, "Kylmä").await;
    let theirs = make_category(&state, other, "Salaiset").await;
    ProductRepo::create(&state.pool, user, "Omena", hevi)
        .await
        .expect("create favorite");
    make_recipe_with(
        &state,
        user,
        "Salaatti",
        &[("Kurkku", None), ("Tomaatti", None)],
        hevi,
    )
    .await;
    make_recipe_with(&state, user, "Tzatziki", &[("Kurkku", None)], hevi).await;
    make_recipe_with(
        &state,
        user,
        "Pannukakku",
        &[("Maito", Some("5 dl")), ("Kerma", None), ("Voi", None)],
        kylma,
    )
    .await;
    make_recipe_with(&state, other, "Heidän", &[("Salainen", None)], theirs).await;
    let secret = any_product_id(&state, other, "Salainen").await;

    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let get = || TestRequest::get().uri("/favorites").cookie(session.clone());
    let post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true")).to_request();

    // The full page: favorites first, then the section with its groups,
    // usage counts, and a "make favorite" form per product.
    let resp = call_service(&app, get().to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let page = body_string(resp).await;
    assert!(page.contains("<html") && page.contains("From recipes"));
    let favorites_at = page.find("Omena").unwrap_or(usize::MAX);
    let section_at = page.find(r#"id="recipe-products""#);
    assert!(
        section_at.is_some_and(|s| favorites_at < s),
        "the section comes after the favorites"
    );
    let section = recipe_section(&page);
    assert!(!section.contains("Omena"), "favorites aren't repeated");
    let hevi_at = section.find("<h3>Hevi</h3>");
    let kylma_at = section.find("<h3>Kylmä</h3>");
    assert!(hevi_at.is_some() && hevi_at < kylma_at, "{section}");
    for name in ["Kurkku", "Tomaatti", "Maito", "Kerma", "Voi"] {
        let id = any_product_id(&state, user, name).await;
        assert!(section.contains(&format!(r#"id="recipe-product-{id}""#)));
        assert!(section.contains(&format!(r#"action="/favorites/{id}/favorite""#)));
    }
    assert!(section.contains("Used in 2 recipes") && section.contains("Used in 1 recipe"));
    assert!(!page.contains("Salainen"), "another user's product");
    assert!(!page.contains(&secret.to_string()));

    let kurkku = any_product_id(&state, user, "Kurkku").await;
    let tomaatti = any_product_id(&state, user, "Tomaatti").await;
    let maito = any_product_id(&state, user, "Maito").await;
    let kerma = any_product_id(&state, user, "Kerma").await;
    let voi = any_product_id(&state, user, "Voi").await;
    let (hevi_s, kylma_s) = (hevi.to_string(), kylma.to_string());

    // Rename: over htmx, then without it. They stay recipe-created.
    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{kurkku}/rename"),
            &[("name", "Kurkut")],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    // The htmx fragment carries the section, too.
    let fragment = body_string(resp).await;
    assert!(!fragment.contains("<html"));
    let section = recipe_section(&fragment);
    assert!(section.contains("Kurkut") && section.contains("Used in 2 recipes"));
    let resp = call_service(
        &app,
        post(
            &format!("/favorites/{voi}/rename"),
            &[("name", "Voi (suolaton)")],
        )
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Change category: over htmx, then without it.
    let resp = call_service(
        &app,
        hx(post(
            &format!("/favorites/{maito}/category"),
            &[("category_id", hevi_s.as_str())],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = call_service(
        &app,
        post(
            &format!("/favorites/{tomaatti}/category"),
            &[("category_id", kylma_s.as_str())],
        )
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let s = |v: &[&str]| v.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        hidden_shape(&state, user).await,
        [
            (Some("Hevi".to_owned()), s(&["Kurkut", "Maito"])),
            (
                Some("Kylmä".to_owned()),
                s(&["Kerma", "Tomaatti", "Voi (suolaton)"])
            ),
        ]
    );

    // Make favorite over htmx: it moves from the section to the favorites.
    let resp = call_service(
        &app,
        hx(post(&format!("/favorites/{kurkku}/favorite"), &[])),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(!body.contains("<html"));
    let at = body.find("Kurkut");
    assert!(
        at.is_some() && at < body.find(r#"id="recipe-products""#),
        "listed with the favorites"
    );
    assert!(!recipe_section(&body).contains("Kurkut"));
    // And without htmx: 303 back to the page.
    let resp = call_service(
        &app,
        post(&format!("/favorites/{maito}/favorite"), &[]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/favorites")
    );
    assert_eq!(
        catalog_shape(&state, user).await,
        [(Some("Hevi".to_owned()), s(&["Omena", "Kurkut", "Maito"]))]
    );

    // Delete: over htmx, then without it.
    let resp = call_service(&app, hx(post(&format!("/favorites/{kerma}/delete"), &[]))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!body_string(resp).await.contains("Kerma"));
    let resp = call_service(
        &app,
        post(&format!("/favorites/{voi}/delete"), &[]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        hidden_shape(&state, user).await,
        [(Some("Kylmä".to_owned()), s(&["Tomaatti"]))]
    );

    // The last one becomes a favorite: the section goes away.
    call_service(
        &app,
        hx(post(&format!("/favorites/{tomaatti}/favorite"), &[])),
    )
    .await;
    let page = body_string(call_service(&app, get().to_request()).await).await;
    assert!(!page.contains(r#"id="recipe-products""#));

    // Another user's product: every action is `NotFound`, nothing changes.
    let secret_s = secret.to_string();
    let attempts = [
        post(&format!("/favorites/{secret}/favorite"), &[]),
        post(&format!("/favorites/{secret}/rename"), &[("name", "Minun")]),
        post(
            &format!("/favorites/{secret}/category"),
            &[("category_id", hevi_s.as_str())],
        ),
        post(&format!("/favorites/{secret}/delete"), &[]),
    ];
    for req in attempts {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
    assert_eq!(
        hidden_shape(&state, other).await,
        [(Some("Salaiset".to_owned()), s(&["Salainen"]))]
    );
    assert_eq!(
        any_product_id(&state, other, "Salainen").await.to_string(),
        secret_s
    );
}

/// P4: a recipe-created product left uncategorized by a category deletion is
/// shown as uncategorized, can't be made a favorite yet, and can be given a
/// category here — after which it is accepted as a recipe ingredient again.
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn hidden_uncategorized_fixable() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::repo::RecipeRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let juomat = make_category(&state, user, "Juomat").await;
    make_recipe_with(&state, user, "Aamiainen", &[("Mehu", None)], juomat).await;
    CategoryRepo::delete(&state.pool, user, juomat)
        .await
        .expect("delete category");
    let mehu = any_product_id(&state, user, "Mehu").await;
    let recipe = make_recipe_with(&state, user, "Smoothie", &[], hevi).await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let add_mehu = || {
        post(
            &format!("/recipes/{recipe}/ingredients"),
            &[("name", "mehu")],
        )
    };

    // Shown as uncategorized, with no "make favorite" form.
    let page = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/favorites")
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    let section = recipe_section(&page);
    assert!(section.contains("<h3>Uncategorized</h3>") && section.contains("Mehu"));
    assert!(section.contains("No category: set one under Edit"));
    assert!(!section.contains(&format!("/favorites/{mehu}/favorite")));

    // The dead end MB-0003 P2 leaves: refused as an ingredient…
    let resp = call_service(&app, add_mehu().to_request()).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(body_string(resp).await.contains("has no category"));
    // …and refused as a favorite, without htmx and with it.
    let resp = call_service(
        &app,
        post(&format!("/favorites/{mehu}/favorite"), &[]).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("choose a category for Mehu"));
    let resp = call_service(
        &app,
        post(&format!("/favorites/{mehu}/favorite"), &[])
            .insert_header(("HX-Request", "true"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(r#"role="alert""#) && body.contains("choose a category for Mehu"));

    // Fix it from the section, over htmx.
    let resp = call_service(
        &app,
        post(
            &format!("/favorites/{mehu}/category"),
            &[("category_id", hevi.to_string().as_str())],
        )
        .insert_header(("HX-Request", "true"))
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let section = recipe_section(&body);
    assert!(section.contains("<h3>Hevi</h3>") && !section.contains("Uncategorized"));
    assert!(section.contains(&format!("/favorites/{mehu}/favorite")));
    assert!(
        !ProductRepo::find_by_name(&state.pool, user, "Mehu")
            .await
            .expect("find")
            .expect("exists")
            .is_favorite,
        "still recipe-created"
    );

    // Now it is accepted as an ingredient, linked to the same product.
    let resp = call_service(&app, add_mehu().to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let ingredients = RecipeRepo::get(&state.pool, user, recipe)
        .await
        .expect("get recipe")
        .ingredients;
    assert_eq!(ingredients.len(), 1);
    assert_eq!(ingredients[0].product_id, Some(mehu));
    assert_eq!(ingredients[0].text, "Mehu");
}

/// P5: deleting a product that recipes use asks for confirmation naming the
/// usage count (and says so next to the button for clients without htmx);
/// afterwards those ingredients keep their name — the current one, after a
/// rename — and quantity, unlinked (RR-0001 constraint 4).
#[cfg(test)]
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn delete_used_product_warns() {
    use actix_web::test::{TestRequest, call_service};
    use kauppa_core::repo::RecipeRepo;

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "u@example.com").await;
    let hevi = make_category(&state, user, "Hevi").await;
    let salaatti = make_recipe_with(
        &state,
        user,
        "Salaatti",
        &[("Tomaatti", Some("2")), ("Kurkku", Some("1 kpl"))],
        hevi,
    )
    .await;
    let tzatziki = make_recipe_with(&state, user, "Tzatziki", &[("Kurkku", Some("½"))], hevi).await;
    let scratch = make_recipe_with(&state, user, "Luonnos", &[("Kerma", None)], hevi).await;
    let kerma_ingredient = RecipeRepo::get(&state.pool, user, scratch)
        .await
        .expect("get recipe")
        .ingredients[0]
        .id;
    RecipeRepo::remove_ingredient(&state.pool, user, scratch, kerma_ingredient)
        .await
        .expect("remove ingredient");
    let kurkku = any_product_id(&state, user, "Kurkku").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let hx_post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .set_form(form)
            .to_request()
    };

    // Renamed first, so the saved ingredient text must follow the new name.
    call_service(
        &app,
        hx_post(
            &format!("/favorites/{kurkku}/rename"),
            &[("name", "Kurkut")],
        ),
    )
    .await;

    let page = body_string(
        call_service(
            &app,
            TestRequest::get()
                .uri("/favorites")
                .cookie(session.clone())
                .to_request(),
        )
        .await,
    )
    .await;
    let section = recipe_section(&page);
    let warning = "Used in 2 recipes. Deleting it keeps the ingredient in those recipes, \
                   by name and quantity, no longer linked to a product.";
    assert!(
        section.contains(&format!(r#"hx-confirm="Delete “Kurkut”? {warning}""#)),
        "{section}"
    );
    assert!(section.contains(&format!("<p class=\"muted\">{warning}</p>")));
    assert!(section.contains("hx-confirm=\"Delete “Tomaatti”? Used in 1 recipe. "));
    // An unused product gets the plain confirmation.
    assert!(section.contains(r#"hx-confirm="Delete “Kerma”?""#));

    let resp = call_service(&app, hx_post(&format!("/favorites/{kurkku}/delete"), &[])).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!body_string(resp).await.contains("Kurkut"));
    assert!(
        ProductRepo::find_by_name(&state.pool, user, "Kurkut")
            .await
            .expect("find")
            .is_none()
    );

    // The ingredients stay, by name and quantity, unlinked.
    let lines = |recipe: Uuid| {
        let state = state.clone();
        async move {
            RecipeRepo::get(&state.pool, user, recipe)
                .await
                .expect("get recipe")
                .ingredients
                .into_iter()
                .map(|i| (i.text, i.quantity, i.product_id.is_some()))
                .collect::<Vec<_>>()
        }
    };
    let line = |name: &str, quantity: &str, linked: bool| {
        (name.to_owned(), Some(quantity.to_owned()), linked)
    };
    assert_eq!(
        lines(salaatti).await,
        [line("Tomaatti", "2", true), line("Kurkut", "1 kpl", false)]
    );
    assert_eq!(lines(tzatziki).await, [line("Kurkut", "½", false)]);

    // The recipe page still lists it by name.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/recipes/{tzatziki}"))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains("Kurkut"));
}
