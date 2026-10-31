//! Named shopping lists — create, open to continue, rename, delete (MB-0007).
//!
//! Thin handlers: parse → call `ShoppingListRepo` → render. The index's
//! mutations answer htmx with the re-rendered `#lists` fragment; plain
//! requests get a 303 (PRG), or the full page when the input was rejected.
//! All endpoints require an authenticated user, and another user's list is a
//! 404. Items are MB-0004's; this module only opens the list they belong to.

use actix_identity::Identity;
use actix_web::http::{StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use kauppa_core::domain::ShoppingList;
use kauppa_core::repo::ShoppingListRepo;
use kauppa_core::{CoreError, CoreResult};

use crate::error::{WebError, WebResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct NameForm {
    name: String,
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
    list: ListCard,
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
    let list = ShoppingListRepo::get(&state.pool, user, path.into_inner()).await?;
    let view = ShowView { list: list.into() };
    let body = if wants_fragment(&req) {
        state.templates.render("lists/_list", &view.list)?
    } else {
        state.templates.render("lists/show", &view)?
    };
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
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
