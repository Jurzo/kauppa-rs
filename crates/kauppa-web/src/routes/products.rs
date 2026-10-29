//! Favorite products — the per-user product catalog (MB-0002).
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

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{Category, Direction, ProductGroup};
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
fn parse_category(raw: &str) -> CoreResult<Uuid> {
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

#[derive(Debug, Serialize)]
struct ProductView {
    id: Uuid,
    name: String,
    picker: Picker,
}

/// A category `<select>`; `has_selection` is false when the placeholder
/// ("Choose a category") should be selected instead.
#[derive(Debug, Serialize)]
struct Picker {
    options: Vec<PickerOption>,
    has_selection: bool,
}

#[derive(Debug, Serialize)]
struct PickerOption {
    id: Uuid,
    name: String,
    selected: bool,
}

impl Picker {
    fn new(categories: &[Category], selected: Option<Uuid>) -> Self {
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
        }
    }
}

impl CatalogView {
    fn new(
        categories: &[Category],
        groups: Vec<ProductGroup>,
        draft: Draft,
        error: Option<String>,
    ) -> Self {
        let groups = groups
            .into_iter()
            .map(|group| GroupView {
                name: group.category.map(|c| c.name),
                products: group
                    .products
                    .into_iter()
                    .map(|p| ProductView {
                        picker: Picker::new(categories, p.category_id),
                        id: p.id,
                        name: p.name,
                    })
                    .collect(),
            })
            .collect();
        Self {
            has_categories: !categories.is_empty(),
            add: AddView {
                name: draft.name,
                picker: Picker::new(categories, draft.category_id),
            },
            groups,
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
    let view = CatalogView::new(&categories, groups, draft, error);
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
