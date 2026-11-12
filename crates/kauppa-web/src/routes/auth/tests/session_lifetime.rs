//! MB-0042 P1/P2 (RR-0026): a sign-in ends after its deadlines, enforced on
//! the server, and use keeps an active one alive. The identity middleware
//! reads the real clock and stores whole seconds, so these tests use
//! deadlines of a few seconds with at least a second of margin.

use std::time::Duration;

use actix_web::cookie::Cookie;
use actix_web::dev::ServiceResponse;
use actix_web::http::StatusCode;
use actix_web::test::{TestRequest, call_service};

use crate::identity_middleware;
use crate::routes::test_support::{login_request, make_user, session_cookie, test_app, test_state};

const EMAIL: &str = "lifetime@example.com";

/// The session cookie a response sets, if it sets one.
fn renewed<B>(resp: &ServiceResponse<B>) -> Option<Cookie<'static>> {
    resp.response()
        .cookies()
        .find(|c| c.name() == "kauppa_sid")
        .map(Cookie::into_owned)
}

async fn pause(secs: f64) {
    tokio::time::sleep(Duration::from_secs_f64(secs)).await;
}

/// P1: a session past the idle deadline, and one past the absolute
/// deadline though in use, is answered as signed out (`/lists` → 401).
#[actix_web::test]
async fn expired_session_is_signed_out() {
    // Idle deadline: 3 s, no visit for 4.5 s. actix-identity stores whole
    // seconds and reads the wall clock, which can jump (WSL2 was seen gaining
    // ~1.1 s in 2.8 s), so every check keeps a margin of a second or more.
    let state = test_state().await;
    make_user(&state, EMAIL).await;
    let app = test_app!(
        state,
        identity_middleware(Duration::from_secs(3), Duration::from_secs(3600))
    );
    let session = session_cookie(&call_service(&app, login_request(EMAIL).to_request()).await);
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/lists")
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "signed in at first");
    let session = renewed(&resp).unwrap_or(session);
    pause(4.5).await;
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/lists")
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "idle deadline");

    // Absolute deadline: 2 s, visited every 0.8 s (inside a 1 h idle
    // deadline), still signed out once 2 s have passed since sign-in.
    let state = test_state().await;
    make_user(&state, EMAIL).await;
    let app = test_app!(
        state,
        identity_middleware(Duration::from_secs(3600), Duration::from_secs(2))
    );
    let mut session = session_cookie(&call_service(&app, login_request(EMAIL).to_request()).await);
    let mut last = StatusCode::OK;
    for _ in 0..5 {
        pause(0.8).await;
        let resp = call_service(
            &app,
            TestRequest::get()
                .uri("/lists")
                .cookie(session.clone())
                .to_request(),
        )
        .await;
        last = resp.status();
        if let Some(next) = renewed(&resp) {
            session = next;
        }
    }
    assert_eq!(last, StatusCode::UNAUTHORIZED, "absolute deadline");
}

/// P2: a session inside both deadlines keeps working, and each visit moves
/// the idle deadline: four visits 2 s apart under a 6 s idle deadline keep
/// it signed in 8 s after sign-in. Each check has about 3 s of margin for
/// whole-second timestamps and wall-clock jumps.
#[actix_web::test]
async fn active_session_stays_signed_in() {
    let state = test_state().await;
    make_user(&state, EMAIL).await;
    let app = test_app!(
        state,
        identity_middleware(Duration::from_secs(6), Duration::from_secs(3600))
    );
    let mut session = session_cookie(&call_service(&app, login_request(EMAIL).to_request()).await);
    for visit in 0..4 {
        pause(2.0).await;
        let resp = call_service(
            &app,
            TestRequest::get()
                .uri("/lists")
                .cookie(session.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK, "visit {visit}");
        session = renewed(&resp).expect("the visit renews the session cookie");
    }
}

/// RR-0026 constraint 2: `main.rs` and the test app build the identity
/// middleware through `identity_middleware`, `main.rs` with the deadline
/// constants, and nothing builds it another way.
#[test]
fn identity_wiring_shared() {
    let main = include_str!("../../../main.rs");
    let support = include_str!("../../test_support.rs");
    assert!(
        main.contains(".wrap(identity_middleware(SESSION_IDLE, SESSION_MAX))"),
        "main.rs wraps the identity middleware with the deadlines"
    );
    assert!(
        support.contains("crate::identity_middleware(crate::SESSION_IDLE, crate::SESSION_MAX)"),
        "the test app defaults to the same deadlines"
    );
    for (file, text) in crate::routes::test_support::source_files()
        .into_iter()
        .chain([("routes/test_support.rs".to_owned(), support.to_owned())])
    {
        for (i, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            let other_build = code.contains("IdentityMiddleware::default()")
                || (file != "main.rs" && code.contains("IdentityMiddleware::builder()"));
            assert!(
                !other_build,
                "{file}:{}: build the identity middleware with identity_middleware",
                i + 1
            );
        }
    }
}
