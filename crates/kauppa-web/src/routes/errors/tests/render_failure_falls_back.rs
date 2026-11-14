//! MB-0056 P5 (RR-0038 sub-answer 1, constraint 7): when the error page
//! can't be rendered, the original answer goes out as it was, with one
//! ERROR line naming the template and nothing of the failure.

use std::sync::Arc;

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use actix_web::web;
use uuid::Uuid;

use crate::routes::test_support::{
    LogCapture, bare_test_app, body_string, login_request, make_user, session_cookie, test_app,
    test_state,
};
use crate::state::AppState;
use crate::templates::Templates;

/// App state whose `errors/page` fails to render (strict mode, a value no
/// view has).
async fn broken_state() -> web::Data<AppState> {
    let state = test_state().await;
    let mut templates = Templates::load().expect("templates");
    templates
        .replace_template("errors/page", "{{no_such_value_zq7}}")
        .expect("register");
    web::Data::new(AppState {
        pool: state.pool.clone(),
        templates: Arc::new(templates),
        origin: state.origin.clone(),
        limiter: state.limiter.clone(),
        cookie_secure: false,
    })
}

#[actix_web::test]
async fn render_failure_falls_back() {
    let _idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let capture = LogCapture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let state = broken_state().await;
    let app = test_app!(state);
    let bare = bare_test_app!(state);
    make_user(&state, "u@example.com").await;
    let s = session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);

    let cases = [
        (
            "unknown URL",
            TestRequest::get().uri("/no/such/page"),
            false,
            StatusCode::NOT_FOUND,
            "",
        ),
        (
            "missing list",
            TestRequest::get()
                .uri(&format!("/lists/{}", Uuid::new_v4()))
                .cookie(s.clone())
                .insert_header(("HX-Request", "true")),
            false,
            StatusCode::NOT_FOUND,
            "not found",
        ),
        (
            "signed out",
            TestRequest::get().uri("/lists"),
            false,
            StatusCode::UNAUTHORIZED,
            "",
        ),
        (
            "cross-site",
            TestRequest::post()
                .uri("/lists")
                .insert_header((header::ORIGIN, "http://evil.example"))
                .set_form([("name", "x")]),
            true,
            StatusCode::FORBIDDEN,
            "forbidden",
        ),
    ];
    for (what, req, use_bare, status, body) in cases {
        let resp = if use_bare {
            call_service(&bare, req.to_request()).await
        } else {
            call_service(&app, req.to_request()).await
        };
        assert_eq!(resp.status(), status, "{what}");
        assert!(
            resp.headers().contains_key(header::CONTENT_SECURITY_POLICY),
            "{what}: CSP"
        );
        assert!(resp.headers().get("hx-retarget").is_none(), "{what}");
        assert_eq!(body_string(resp).await, body, "{what}");
    }

    let log = capture.text();
    let lines: Vec<&str> = log
        .lines()
        .filter(|l| l.contains("error page not rendered"))
        .collect();
    assert_eq!(lines.len(), 4, "{log}");
    for line in lines {
        assert!(line.contains("ERROR"), "{line}");
        assert!(line.contains("errors/page"), "{line}");
        assert!(!line.contains("no_such_value_zq7"), "no detail: {line}");
    }
}
