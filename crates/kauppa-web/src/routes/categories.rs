//! Category management endpoints (MB-0001).
//!
//! Thin handlers: parse → call `CategoryRepo` → render. htmx requests get the
//! re-rendered `#categories` fragment; plain requests get a 303 back to the
//! page (PRG), or the full page when the input was rejected. All endpoints
//! require an authenticated user.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::{Category, Direction, ordering};
use kauppa_core::repo::CategoryRepo;
use kauppa_core::{CoreError, CoreResult};

use crate::error::{WebError, WebResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct NameForm {
    name: String,
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

#[derive(Debug, Serialize)]
struct CategoriesView {
    categories: Vec<Category>,
    /// Pre-fills the "new category" field after a rejected add.
    draft_name: String,
    error: Option<String>,
}

/// Render the page or, for htmx, the `#categories` fragment.
async fn render(
    state: &AppState,
    user: Uuid,
    fragment: bool,
    status: StatusCode,
    draft_name: String,
    error: Option<String>,
) -> WebResult<HttpResponse> {
    let categories = state.pool.list_for_owner(user).await?;
    let view = CategoriesView {
        categories,
        draft_name,
        error,
    };
    let template = if fragment {
        "categories/_section"
    } else {
        "categories/index"
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
    draft_name: String,
) -> WebResult<HttpResponse> {
    let (status, error) = match outcome {
        Ok(_) => (StatusCode::OK, None),
        Err(CoreError::Invalid(m)) => (StatusCode::BAD_REQUEST, Some(m)),
        Err(CoreError::Conflict(m)) => (StatusCode::CONFLICT, Some(m)),
        Err(e) => return Err(e.into()),
    };
    if let Some(reason) = &error {
        tracing::debug!(user_id = %user, %reason, "categories input rejected");
    }

    if is_hx(req) {
        render(state, user, true, StatusCode::OK, draft_name, error).await
    } else if error.is_none() {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, "/categories"))
            .finish())
    } else {
        render(state, user, false, status, draft_name, error).await
    }
}

#[get("")]
pub async fn index(state: web::Data<AppState>, id: Identity) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    render(&state, user, false, StatusCode::OK, String::new(), None).await
}

#[post("")]
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    form: web::Form<NameForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = state.pool.create(user, &form.name).await;
    let draft_name = if outcome.is_ok() {
        String::new()
    } else {
        form.name.clone()
    };
    respond(&state, &req, user, outcome, draft_name).await
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
    let outcome = state.pool.rename(user, path.into_inner(), &form.name).await;
    respond(&state, &req, user, outcome, String::new()).await
}

#[post("/{id}/delete")]
pub async fn delete(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let outcome = state.pool.delete(user, path.into_inner()).await;
    respond(&state, &req, user, outcome, String::new()).await
}

/// Move a category one step up or down, then persist the new ordering.
#[post("/{id}/move")]
pub async fn move_category(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
    form: web::Form<MoveForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let target = path.into_inner();

    let mut ids: Vec<Uuid> = state
        .pool
        .list_for_owner(user)
        .await?
        .iter()
        .map(|c| c.id)
        .collect();
    let at = ids
        .iter()
        .position(|&c| c == target)
        .ok_or(CoreError::NotFound)?;
    if ordering::step(&mut ids, at, form.dir) {
        state.pool.reorder(user, &ids).await?;
    }

    respond(&state, &req, user, Ok(()), String::new()).await
}

// MB-0001 acceptance test (P6). It sits at module level rather than in a
// `tests` submodule so the brief's `cargo test -p kauppa-web
// routes::categories::crud_and_auth` check selects it.

/// The user's category names, in display order.
#[cfg(test)]
#[allow(clippy::expect_used)]
async fn category_names(state: &AppState, user: Uuid) -> Vec<String> {
    state
        .pool
        .list_for_owner(user)
        .await
        .expect("list categories")
        .into_iter()
        .map(|c| c.name)
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used)]
async fn category_id(state: &AppState, user: Uuid, name: &str) -> Uuid {
    state
        .pool
        .list_for_owner(user)
        .await
        .expect("list categories")
        .into_iter()
        .find(|c| c.name == name)
        .expect("category exists")
        .id
}

/// P6: `GET /categories` renders the owner's categories; add / rename / move /
/// delete return the fragment over htmx and the full page (via 303, or with
/// the error) otherwise; unauthenticated requests are rejected.
#[cfg(test)]
#[actix_web::test]
async fn crud_and_auth() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_category, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);

    // Unauthenticated: every route is rejected.
    let any = Uuid::new_v4();
    let anonymous = [
        TestRequest::get().uri("/categories"),
        TestRequest::post()
            .uri("/categories")
            .set_form([("name", "Hevi")]),
        TestRequest::post()
            .uri(&format!("/categories/{any}/rename"))
            .set_form([("name", "Hevi")]),
        TestRequest::post()
            .uri(&format!("/categories/{any}/move"))
            .set_form([("dir", "up")]),
        TestRequest::post().uri(&format!("/categories/{any}/delete")),
    ];
    for req in anonymous {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    let user = make_user(&state, "u@example.com").await;
    let other = make_user(&state, "other@example.com").await;
    let theirs = make_category(&state, other, "Pakasteet").await;
    let session =
        session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let post = |uri: &str, form: &[(&str, &str)]| {
        TestRequest::post()
            .uri(uri)
            .cookie(session.clone())
            .set_form(form)
    };
    let hx = |req: TestRequest| req.insert_header(("HX-Request", "true")).to_request();

    // The page: a full document whose forms work without JS.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/categories")
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("Categories"));
    assert!(body.contains(r#"method="post" action="/categories""#));
    assert!(!body.contains("Pakasteet"), "another owner's category");

    // Add over htmx: the fragment, with the new category.
    let resp = call_service(&app, hx(post("/categories", &[("name", "Hevi")]))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(
        !body.contains("<html"),
        "htmx response should be a fragment"
    );
    assert!(body.contains(r#"id="categories""#) && body.contains(r#"value="Hevi""#));

    // Add without htmx: 303 back to the page.
    let resp = call_service(&app, post("/categories", &[("name", "Kylmä")]).to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/categories")
    );

    // Rejected input is shown, not dropped: over htmx as the fragment with an
    // alert; without htmx as the full page with the 4xx status and the typed
    // name kept.
    let resp = call_service(&app, hx(post("/categories", &[("name", "  ")]))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    assert!(body.contains(r#"role="alert""#) && body.contains("must not be empty"));
    let resp = call_service(&app, post("/categories", &[("name", "Hevi")]).to_request()).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(body.contains("<html") && body.contains("category already exists: Hevi"));
    assert!(
        body.contains(r#"id="new-category" name="name""#) && body.contains(r#"value="Hevi" />"#)
    );
    assert_eq!(category_names(&state, user).await, ["Hevi", "Kylmä"]);

    let hevi = category_id(&state, user, "Hevi").await;

    // Rename, move, and delete over htmx.
    let resp = call_service(
        &app,
        hx(post(
            &format!("/categories/{hevi}/rename"),
            &[("name", "Hedelmät")],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(body_string(resp).await.contains(r#"value="Hedelmät""#));

    let resp = call_service(
        &app,
        hx(post(
            &format!("/categories/{hevi}/move"),
            &[("dir", "down")],
        )),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_string(resp).await;
    let (moved, other_row) = (body.find("Hedelmät"), body.find("Kylmä"));
    assert!(
        moved.is_some() && other_row < moved,
        "Hedelmät moved below Kylmä"
    );
    assert_eq!(category_names(&state, user).await, ["Kylmä", "Hedelmät"]);

    let resp = call_service(&app, hx(post(&format!("/categories/{hevi}/delete"), &[]))).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(!body_string(resp).await.contains("Hedelmät"));

    // Without htmx, edits redirect back to the page; another owner's category
    // is not found.
    let kylma = category_id(&state, user, "Kylmä").await;
    let resp = call_service(
        &app,
        post(
            &format!("/categories/{kylma}/rename"),
            &[("name", "Kylmät")],
        )
        .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let resp = call_service(&app, hx(post(&format!("/categories/{theirs}/delete"), &[]))).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(category_names(&state, user).await, ["Kylmät"]);
    assert_eq!(category_names(&state, other).await, ["Pakasteet"]);
}
