//! Category management endpoints (MB-0001).
//!
//! Thin handlers: parse → call `CategoryRepo` → render. htmx-triggered requests
//! get an HTML fragment of the list; plain requests get the full page or a
//! 303-redirect back to it (PRG). All endpoints require an authenticated user.

use actix_identity::Identity;
use actix_web::{HttpRequest, HttpResponse, get, http::header, post, web};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use kauppa_core::CoreError;
use kauppa_core::repo::CategoryRepo;

use crate::error::{WebError, WebResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct NameForm {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MoveForm {
    dir: String,
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

/// htmx → the re-rendered list fragment; otherwise a 303 back to the page.
async fn list_fragment_or_redirect(
    state: &web::Data<AppState>,
    req: &HttpRequest,
    user: Uuid,
) -> WebResult<HttpResponse> {
    if is_hx(req) {
        let categories = state.pool.list_for_owner(user).await?;
        let body = state
            .templates
            .render("categories/_list", &json!({ "categories": categories }))?;
        Ok(HttpResponse::Ok().content_type("text/html").body(body))
    } else {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, "/categories"))
            .finish())
    }
}

#[get("")]
pub async fn index(state: web::Data<AppState>, id: Identity) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    let categories = state.pool.list_for_owner(user).await?;
    let body = state
        .templates
        .render("categories/index", &json!({ "categories": categories }))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}

#[post("")]
pub async fn create(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    form: web::Form<NameForm>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    state.pool.create(user, &form.name).await?;
    list_fragment_or_redirect(&state, &req, user).await
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
    state
        .pool
        .rename(user, path.into_inner(), &form.name)
        .await?;
    list_fragment_or_redirect(&state, &req, user).await
}

#[post("/{id}/delete")]
pub async fn delete(
    req: HttpRequest,
    state: web::Data<AppState>,
    id: Identity,
    path: web::Path<Uuid>,
) -> WebResult<HttpResponse> {
    let user = current_user(&id)?;
    state.pool.delete(user, path.into_inner()).await?;
    list_fragment_or_redirect(&state, &req, user).await
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

    let mut categories = state.pool.list_for_owner(user).await?;
    let idx = categories
        .iter()
        .position(|c| c.id == target)
        .ok_or(CoreError::NotFound)?;

    let swap_with = match form.dir.as_str() {
        "up" if idx > 0 => Some(idx - 1),
        "down" if idx + 1 < categories.len() => Some(idx + 1),
        _ => None,
    };
    if let Some(j) = swap_with {
        categories.swap(idx, j);
        let ordered: Vec<Uuid> = categories.iter().map(|c| c.id).collect();
        state.pool.reorder(user, &ordered).await?;
    }

    list_fragment_or_redirect(&state, &req, user).await
}

#[cfg(test)]
mod tests {
    //! MB-0001 P6: route rendering, htmx fragments, and the unauthenticated path.
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Arc;

    use actix_identity::IdentityMiddleware;
    use actix_session::SessionMiddleware;
    use actix_session::storage::CookieSessionStore;
    use actix_web::cookie::{Cookie, Key};
    use actix_web::{App, http::StatusCode, test, web};

    use kauppa_core::password;
    use kauppa_core::repo::{CategoryRepo, SqlitePool, UserRepo};

    use crate::routes;
    use crate::state::AppState;

    const PASSWORD: &str = "correct horse battery staple";

    async fn test_state() -> (web::Data<AppState>, uuid::Uuid) {
        let url = format!(
            "sqlite:file:catweb-{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        let pool = SqlitePool::connect(&url).await.unwrap();
        pool.run_migrations().await.unwrap();
        let hash = password::hash(PASSWORD).unwrap();
        let user = UserRepo::create(&pool, "u@example.com", "U", &hash)
            .await
            .unwrap();

        // Build the registry from an absolute path: `cargo test` runs with the
        // crate dir as CWD, so the production `./templates` would not resolve.
        let mut hb = handlebars::Handlebars::new();
        hb.set_strict_mode(true);
        hb.register_templates_directory(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates"),
            handlebars::DirectorySourceOptions::default(),
        )
        .unwrap();

        let state = web::Data::new(AppState {
            pool,
            templates: Arc::new(hb),
        });
        (state, user.id)
    }

    macro_rules! build_app {
        ($state:expr) => {
            test::init_service(
                App::new()
                    .app_data($state.clone())
                    .wrap(IdentityMiddleware::default())
                    .wrap(
                        SessionMiddleware::builder(
                            CookieSessionStore::default(),
                            Key::from(&[0u8; 64]),
                        )
                        .cookie_name("kauppa_sid".to_owned())
                        .cookie_secure(false)
                        .build(),
                    )
                    .configure(routes::configure),
            )
            .await
        };
    }

    /// P6: an unauthenticated request to the page is rejected.
    #[actix_web::test]
    async fn index_requires_auth() {
        let (state, _) = test_state().await;
        let app = build_app!(state);

        let resp = test::call_service(
            &app,
            test::TestRequest::get().uri("/categories").to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    /// P6: signed in, GET renders the full page and the mutations return
    /// fragments over htmx (full-page redirect without it).
    #[actix_web::test]
    async fn crud_and_auth() {
        let (state, user) = test_state().await;
        let app = build_app!(state.clone());

        // Log in and capture the session cookie.
        let login = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/auth/login")
                .set_form([("email", "u@example.com"), ("password", PASSWORD)])
                .to_request(),
        )
        .await;
        assert_eq!(login.status(), StatusCode::SEE_OTHER);
        let session: Cookie<'static> = login
            .response()
            .cookies()
            .find(|c| c.name() == "kauppa_sid")
            .expect("session cookie")
            .into_owned();

        // GET the page: full HTML document.
        let page = test::call_service(
            &app,
            test::TestRequest::get()
                .uri("/categories")
                .cookie(session.clone())
                .to_request(),
        )
        .await;
        assert_eq!(page.status(), StatusCode::OK);
        let page_body = String::from_utf8(test::read_body(page).await.to_vec()).unwrap();
        assert!(
            page_body.contains("<html"),
            "page should be a full document"
        );
        assert!(page_body.contains("Categories"));

        // Create over htmx: a fragment (no <html>) containing the new name.
        let created = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/categories")
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .set_form([("name", "Hevi")])
                .to_request(),
        )
        .await;
        assert_eq!(created.status(), StatusCode::OK);
        let frag = String::from_utf8(test::read_body(created).await.to_vec()).unwrap();
        assert!(
            !frag.contains("<html"),
            "htmx response should be a fragment"
        );
        assert!(frag.contains("id=\"category-list\""));
        assert!(frag.contains("Hevi"));

        // Create without htmx: 303 redirect back to the page.
        let redirect = test::call_service(
            &app,
            test::TestRequest::post()
                .uri("/categories")
                .cookie(session.clone())
                .set_form([("name", "Kylma")])
                .to_request(),
        )
        .await;
        assert_eq!(redirect.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            redirect
                .headers()
                .get(actix_web::http::header::LOCATION)
                .unwrap(),
            "/categories"
        );

        // Grab the created ids straight from the repo to drive rename/move/delete.
        let cats = CategoryRepo::list_for_owner(&state.pool, user)
            .await
            .unwrap();
        assert_eq!(cats.len(), 2);
        let hevi = cats.iter().find(|c| c.name == "Hevi").unwrap().id;

        // Rename over htmx.
        let renamed = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/categories/{hevi}/rename"))
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .set_form([("name", "Pakasteet")])
                .to_request(),
        )
        .await;
        assert_eq!(renamed.status(), StatusCode::OK);
        let renamed_body = String::from_utf8(test::read_body(renamed).await.to_vec()).unwrap();
        assert!(renamed_body.contains("Pakasteet"));
        assert!(!renamed_body.contains(">Hevi<"));

        // Move over htmx (reorder).
        let moved = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/categories/{hevi}/move"))
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .set_form([("dir", "down")])
                .to_request(),
        )
        .await;
        assert_eq!(moved.status(), StatusCode::OK);

        // Delete over htmx; the name is gone from the fragment afterwards.
        let deleted = test::call_service(
            &app,
            test::TestRequest::post()
                .uri(&format!("/categories/{hevi}/delete"))
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .to_request(),
        )
        .await;
        assert_eq!(deleted.status(), StatusCode::OK);
        let deleted_body = String::from_utf8(test::read_body(deleted).await.to_vec()).unwrap();
        assert!(!deleted_body.contains("Pakasteet"));

        let remaining = CategoryRepo::list_for_owner(&state.pool, user)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "Kylma");
    }
}
