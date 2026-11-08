//! Fixtures shared by route tests: app state over an isolated in-memory
//! database, an app wired like `main.rs`, and a signed-in session.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use actix_web::body::MessageBody;
use actix_web::cookie::Cookie;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::{self, HeaderValue};
use actix_web::{test, web};
use uuid::Uuid;

use kauppa_core::domain::{NewIngredient, RecipeInput};
use kauppa_core::password;
use kauppa_core::repo::{CategoryRepo, RecipeRepo, ShoppingListRepo, SqlitePool, UserRepo};

use crate::csrf::{OriginPolicy, Scheme};
use crate::state::AppState;
use crate::templates;

pub const PASSWORD: &str = "correct horse battery staple";

/// App state over a fresh in-memory database and the embedded templates.
pub async fn test_state() -> web::Data<AppState> {
    let url = format!(
        "sqlite:file:web-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    let templates = templates::build_registry().expect("load templates");
    web::Data::new(AppState {
        pool,
        templates: Arc::new(templates),
        // As `main.rs` with `KAUPPA_PUBLIC_ORIGIN` unset and cookies not
        // `Secure`: the request's own `Host`, over http.
        origin: OriginPolicy::SameHost(Scheme::Http),
    })
}

/// The host `TestRequest` reports by default (actix-web's test config), and
/// the origin a page served from it has.
pub const TEST_HOST: &str = "localhost:8080";
pub const TEST_ORIGIN: &str = "http://localhost:8080";

/// Gives a request the proof a same-origin browser sends (RR-0010
/// constraint 4): when it carries neither `Origin` nor `Referer`, it gets
/// `Origin` for its `Host` (and `Host: localhost:8080` if it has none).
/// `test_app!` applies this to every request, so route tests send valid
/// proof by default; a test that sets its own `Origin` or `Referer` keeps
/// it, and the CSRF tests use `bare_test_app!`, which never adds anything.
pub fn add_default_proof(req: &mut ServiceRequest) {
    let headers = req.headers_mut();
    if headers.contains_key(header::ORIGIN) || headers.contains_key(header::REFERER) {
        return;
    }
    if !headers.contains_key(header::HOST) {
        headers.insert(header::HOST, HeaderValue::from_static(TEST_HOST));
    }
    let host = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .expect("ascii host");
    let origin = HeaderValue::from_str(&format!("http://{host}")).expect("header value");
    headers.insert(header::ORIGIN, origin);
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

/// A recipe titled `title` with no ingredients; returns its id.
pub async fn make_recipe(state: &AppState, owner: Uuid, title: &str) -> Uuid {
    let input = RecipeInput {
        title: title.to_owned(),
        ..RecipeInput::default()
    };
    RecipeRepo::create(&state.pool, owner, &input)
        .await
        .expect("create recipe")
        .id
}

/// A recipe with one ingredient per `(product name, quantity)`; new products
/// go to `category`.
pub async fn make_recipe_with(
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

pub async fn make_list(state: &AppState, owner: Uuid, name: &str) -> Uuid {
    ShoppingListRepo::create(&state.pool, owner, name)
        .await
        .expect("create list")
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

/// A `tracing` writer that keeps everything written to it.
#[derive(Debug, Clone, Default)]
pub struct LogCapture(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl LogCapture {
    pub fn text(&self) -> String {
        String::from_utf8(self.0.lock().expect("log lock").clone()).expect("utf-8 log")
    }
}

impl std::io::Write for LogCapture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("log lock").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for LogCapture {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// The `App` as `main.rs` wires it: identity + cookie session + all routes,
/// which `routes::configure` puts behind the cross-site check. Not
/// initialized; see [`test_app!`] and [`bare_test_app!`].
macro_rules! wired_app {
    ($state:expr) => {
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
            .configure(crate::routes::configure)
    };
}
pub(crate) use wired_app;

/// An app wired like `main.rs`, whose requests carry same-origin proof
/// unless the test sets its own ([`add_default_proof`]). A macro because
/// `init_service` returns an unnameable `impl Service`.
macro_rules! test_app {
    ($state:expr) => {
        actix_web::test::init_service(crate::routes::test_support::wired_app!($state).wrap_fn(
            |mut req, srv| {
                crate::routes::test_support::add_default_proof(&mut req);
                actix_web::dev::Service::call(srv, req)
            },
        ))
        .await
    };
}
pub(crate) use test_app;

/// An app wired like `main.rs` that sends requests exactly as built: no
/// default proof. For the cross-site tests (MB-0018), which strip or forge it.
macro_rules! bare_test_app {
    ($state:expr) => {
        actix_web::test::init_service(crate::routes::test_support::wired_app!($state)).await
    };
}
pub(crate) use bare_test_app;
