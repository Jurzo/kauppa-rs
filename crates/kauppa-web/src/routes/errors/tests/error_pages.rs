//! MB-0056 P1: every error answer inside the RR-0010 scope is a page in
//! the layout, in the request's language and theme, with the status it had.

use actix_web::cookie::Cookie;
use actix_web::http::{StatusCode, header};
use actix_web::middleware::from_fn;
use actix_web::test::{TestRequest, call_service, init_service};
use actix_web::{HttpResponse, web};
use uuid::Uuid;

use kauppa_core::domain::{Language, ShareRole};
use kauppa_core::repo::ShoppingListRepo;

use super::{
    BAD_REQUEST, FORBIDDEN, NOT_FOUND, OTHER, SERVER, SIGNED_OUT, assert_html, assert_page,
    main_of, shown,
};
use crate::error::WebError;
use crate::routes::errors;
use crate::routes::test_support::{
    bare_test_app, body_string, login_request, make_list, make_user, session_cookie, test_app,
    test_state,
};

/// One request per source of an error: the router, the extractors, the
/// handlers' `WebError`, authorization, the cross-site check.
#[actix_web::test]
async fn every_source() {
    let state = test_state().await;
    let app = test_app!(state);
    let bare = bare_test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    make_user(&state, "viewer@example.com").await;
    let list = make_list(&state, owner, "Kauppa").await;
    ShoppingListRepo::share(
        &state.pool,
        owner,
        list,
        "viewer@example.com",
        ShareRole::View,
    )
    .await
    .expect("share")
    .expect("viewer matched");
    let viewer =
        session_cookie(&call_service(&app, login_request("viewer@example.com").to_request()).await);
    let missing = format!("/lists/{}", Uuid::new_v4());

    let cases: Vec<(&str, TestRequest, bool, StatusCode, &str, bool)> = vec![
        (
            "unknown URL",
            TestRequest::get().uri("/no/such/page"),
            false,
            StatusCode::NOT_FOUND,
            NOT_FOUND,
            false,
        ),
        (
            "missing list",
            TestRequest::get().uri(&missing).cookie(viewer.clone()),
            false,
            StatusCode::NOT_FOUND,
            NOT_FOUND,
            true,
        ),
        (
            "malformed id",
            TestRequest::get()
                .uri("/lists/not-a-uuid")
                .cookie(viewer.clone()),
            false,
            StatusCode::NOT_FOUND,
            NOT_FOUND,
            true,
        ),
        (
            "signed out",
            TestRequest::get().uri("/lists"),
            false,
            StatusCode::UNAUTHORIZED,
            SIGNED_OUT,
            false,
        ),
        (
            "not allowed",
            TestRequest::get()
                .uri(&format!("/lists/{list}/sharing"))
                .cookie(viewer.clone()),
            false,
            StatusCode::FORBIDDEN,
            FORBIDDEN,
            true,
        ),
        (
            "cross-site",
            TestRequest::post()
                .uri("/lists")
                .cookie(viewer.clone())
                .insert_header((header::ORIGIN, "http://evil.example"))
                .set_form([("name", "x")]),
            true,
            StatusCode::FORBIDDEN,
            FORBIDDEN,
            true,
        ),
        (
            "rejected value",
            TestRequest::post()
                .uri("/theme")
                .set_form([("theme", "sepia"), ("return", "/")]),
            false,
            StatusCode::BAD_REQUEST,
            BAD_REQUEST,
            false,
        ),
        (
            "malformed form",
            TestRequest::post()
                .uri("/lists")
                .cookie(viewer.clone())
                .set_form([("unexpected", "x")]),
            false,
            StatusCode::BAD_REQUEST,
            BAD_REQUEST,
            true,
        ),
        (
            "wrong method",
            TestRequest::get().uri("/auth/logout"),
            false,
            StatusCode::METHOD_NOT_ALLOWED,
            OTHER,
            false,
        ),
    ];
    for (what, req, use_bare, status, kind, signed_in) in cases {
        let resp = if use_bare {
            call_service(&bare, req.to_request()).await
        } else {
            call_service(&app, req.to_request()).await
        };
        assert_eq!(resp.status(), status, "{what}");
        assert_html(&resp, what);
        let body = body_string(resp).await;
        assert_page(&state, &body, Language::En, kind, what);
        // Signed in: a way to the lists, and the navigation's sign-out.
        let main = main_of(&body);
        assert_eq!(
            main.contains(r#"href="/lists""#),
            signed_in,
            "{what}: the lists link\n{main}"
        );
        assert_eq!(
            body.contains(r#"action="/auth/logout""#),
            signed_in,
            "{what}: navigation"
        );
    }
}

/// The request's language and theme, as on every other page.
#[actix_web::test]
async fn language_and_theme() {
    let state = test_state().await;
    let app = test_app!(state);
    for (lang, accept) in [(Language::En, "en"), (Language::Fi, "fi-FI, en;q=0.5")] {
        for theme in ["light", "dark"] {
            let resp = call_service(
                &app,
                TestRequest::get()
                    .uri("/no/such/page")
                    .insert_header((header::ACCEPT_LANGUAGE, accept))
                    .cookie(Cookie::new("kauppa_theme", theme))
                    .to_request(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
            let body = body_string(resp).await;
            assert_page(&state, &body, lang, NOT_FOUND, accept);
            assert!(
                body.contains(&format!(r#"id="theme-{theme}" checked"#)),
                "{accept} {theme}: the theme switch shows the choice"
            );
        }
    }
}

/// A server fault is a page too, and says nothing of the fault.
#[actix_web::test]
async fn server_error() {
    let state = test_state().await;
    let app = init_service(actix_web::App::new().app_data(state.clone()).service(
        web::scope("").wrap(from_fn(errors::pages)).route(
            "/boom",
            web::get().to(|| async {
                Err::<HttpResponse, _>(WebError::Internal("secret detail".to_owned()))
            }),
        ),
    ))
    .await;
    let resp = call_service(&app, TestRequest::get().uri("/boom").to_request()).await;
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = body_string(resp).await;
    assert_page(&state, &body, Language::En, SERVER, "500");
    assert!(!body.contains("secret detail"), "{body}");
}

/// What a handler renders on purpose stays as it is: the login form's 401
/// with its message, and `/healthz`.
#[actix_web::test]
async fn handler_pages_untouched() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "u@example.com").await;
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/auth/login")
            .set_form([("email", "u@example.com"), ("password", "wrong password!")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = body_string(resp).await;
    assert!(
        body.contains(&shown(&state, Language::En, "auth.invalid_login")),
        "{body}"
    );
    assert!(
        !body.contains(&shown(&state, Language::En, "error_page.signed_out.title")),
        "the login page, not the error page"
    );

    let resp = call_service(&app, TestRequest::get().uri("/healthz").to_request()).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(body_string(resp).await, "ok");
}
