//! Fixtures shared by route tests: app state over an isolated in-memory
//! database, an app wired like `main.rs`, and a signed-in session.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use actix_web::body::MessageBody;
use actix_web::cookie::Cookie;
use actix_web::dev::ServiceResponse;
use actix_web::{test, web};
use uuid::Uuid;

use kauppa_core::password;
use kauppa_core::repo::{CategoryRepo, SqlitePool, UserRepo};

use crate::state::AppState;
use crate::templates;

pub const PASSWORD: &str = "correct horse battery staple";

/// App state over a fresh in-memory database and the workspace's templates.
pub async fn test_state() -> web::Data<AppState> {
    let url = format!(
        "sqlite:file:web-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    let templates =
        templates::build_registry_from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../templates"))
            .expect("load templates");
    web::Data::new(AppState {
        pool,
        templates: Arc::new(templates),
    })
}

/// A user whose password is [`PASSWORD`]; returns their id.
pub async fn make_user(state: &AppState, email: &str) -> Uuid {
    let hash = password::hash(PASSWORD).expect("hash password");
    UserRepo::create(&state.pool, email, "Test User", &hash)
        .await
        .expect("create user")
        .id
}

pub async fn make_category(state: &AppState, owner: Uuid, name: &str) -> Uuid {
    CategoryRepo::create(&state.pool, owner, name)
        .await
        .expect("create category")
        .id
}

/// A login request for a user made by [`make_user`].
pub fn login_request(email: &str) -> test::TestRequest {
    test::TestRequest::post()
        .uri("/auth/login")
        .set_form([("email", email), ("password", PASSWORD)])
}

/// The session cookie set by a successful login response.
pub fn session_cookie<B>(resp: &ServiceResponse<B>) -> Cookie<'static> {
    resp.response()
        .cookies()
        .find(|c| c.name() == "kauppa_sid")
        .expect("session cookie")
        .into_owned()
}

pub async fn body_string<B: MessageBody>(resp: ServiceResponse<B>) -> String {
    String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf-8 body")
}

/// An app wired like `main.rs`: identity + cookie session + all routes. A macro
/// because `init_service` returns an unnameable `impl Service`.
macro_rules! test_app {
    ($state:expr) => {
        actix_web::test::init_service(
            actix_web::App::new()
                .app_data($state.clone())
                .wrap(actix_identity::IdentityMiddleware::default())
                .wrap(
                    actix_session::SessionMiddleware::builder(
                        actix_session::storage::CookieSessionStore::default(),
                        actix_web::cookie::Key::from(&[0u8; 64]),
                    )
                    .cookie_name("kauppa_sid".to_owned())
                    .cookie_secure(false)
                    .build(),
                )
                .configure(crate::routes::configure),
        )
        .await
    };
}
pub(crate) use test_app;
