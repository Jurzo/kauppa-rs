//! Fixtures shared by route tests: app state over an isolated in-memory
//! database, an app wired like `main.rs`, and a signed-in session.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
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
use crate::routes::auth::limiter::{AuthLimiter, FakeClock, TrustedProxies};
use crate::state::AppState;
use crate::templates;

pub const PASSWORD: &str = "correct horse battery staple";

/// App state over a fresh in-memory database and the embedded templates.
pub async fn test_state() -> web::Data<AppState> {
    test_state_with_url().await.0
}

/// [`test_state`], plus the database URL, for a test that opens a second
/// connection to change rows no repo method changes.
pub async fn test_state_with_url() -> (web::Data<AppState>, String) {
    let (state, url, _) = test_state_with_clock().await;
    (state, url)
}

/// [`test_state_with_url`], plus the limiter's clock, which only moves when
/// a test advances it (RR-0013 constraint 7).
pub async fn test_state_with_clock() -> (web::Data<AppState>, String, Arc<FakeClock>) {
    let url = format!(
        "sqlite:file:web-{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let pool = SqlitePool::connect(&url).await.expect("connect test pool");
    pool.run_migrations().await.expect("run migrations");
    let templates = templates::Templates::load().expect("load templates");
    let clock = Arc::new(FakeClock::default());
    let state = web::Data::new(AppState {
        pool,
        templates: Arc::new(templates),
        // As `main.rs` with `KAUPPA_PUBLIC_ORIGIN` unset and cookies not
        // `Secure`: the request's own `Host`, over http.
        origin: OriginPolicy::SameHost(Scheme::Http),
        // As `main.rs` with `KAUPPA_TRUSTED_PROXIES` unset.
        limiter: Arc::new(AuthLimiter::new(clock.clone(), TrustedProxies::default())),
        cookie_secure: false,
    });
    (state, url, clock)
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

/// Whether `resp` leaves the session as it was: it sets no cookie other than
/// `kauppa_sid`, and doesn't remove that one. Since MB-0042 (RR-0026) the
/// identity middleware renews the session cookie's last-visit time on every
/// signed-in request, before any guard or handler runs, so a renewed
/// `kauppa_sid` is expected; a new identity or a purge is not.
pub fn session_kept<B>(resp: &ServiceResponse<B>) -> bool {
    resp.response().cookies().all(|c| {
        c.name() == "kauppa_sid"
            && !c.value().is_empty()
            && c.max_age() != Some(actix_web::cookie::time::Duration::ZERO)
    })
}

pub async fn body_string<B: MessageBody>(resp: ServiceResponse<B>) -> String {
    String::from_utf8(test::read_body(resp).await.to_vec()).expect("utf-8 body")
}

/// Every non-test `.rs` file under the crate's `src/`, as `(path relative to
/// src, text)`: skips `tests/` directories and this file. For the source
/// scans (`csrf::tests::coverage`, `routes::auth`'s bypass guard).
pub fn source_files() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read src dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n != "tests") {
                    walk(&path, out);
                }
            } else if path.extension().is_some_and(|e| e == "rs")
                && path.file_name().is_some_and(|n| n != "test_support.rs")
            {
                out.push(path);
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut paths = Vec::new();
    walk(&src, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(&src).expect("under src");
            let rel = rel.to_string_lossy().replace('\\', "/");
            (rel, std::fs::read_to_string(&p).expect("read source"))
        })
        .collect()
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

/// The `App` as `main.rs` wires it: identity + cookie session + security
/// headers (outermost) + all routes, which `routes::configure` puts behind
/// the cross-site check. Not initialized; see [`test_app!`] and
/// [`bare_test_app!`].
macro_rules! wired_app {
    ($state:expr) => {
        crate::routes::test_support::wired_app!(
            $state,
            crate::identity_middleware(crate::SESSION_IDLE, crate::SESSION_MAX)
        )
    };
    // The identity middleware with other deadlines: only the durations may
    // differ from `main.rs`, and only in tests (RR-0026 constraint 2).
    ($state:expr, $identity:expr) => {
        actix_web::App::new()
            .app_data($state.clone())
            .wrap($identity)
            .wrap(
                actix_session::SessionMiddleware::builder(
                    actix_session::storage::CookieSessionStore::default(),
                    actix_web::cookie::Key::from(&[0u8; 64]),
                )
                .cookie_name("kauppa_sid".to_owned())
                .cookie_secure(false)
                .build(),
            )
            // As main.rs: the security headers, outermost (RR-0019).
            .wrap(crate::csp::headers(false))
            .configure(crate::routes::configure)
    };
}
pub(crate) use wired_app;

/// An app wired like `main.rs`, whose requests carry same-origin proof
/// unless the test sets its own ([`add_default_proof`]). A macro because
/// `init_service` returns an unnameable `impl Service`.
macro_rules! test_app {
    ($state:expr) => {
        crate::routes::test_support::test_app!(
            $state,
            crate::identity_middleware(crate::SESSION_IDLE, crate::SESSION_MAX)
        )
    };
    ($state:expr, $identity:expr) => {
        actix_web::test::init_service(
            crate::routes::test_support::wired_app!($state, $identity).wrap_fn(|mut req, srv| {
                crate::routes::test_support::add_default_proof(&mut req);
                actix_web::dev::Service::call(srv, req)
            }),
        )
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
