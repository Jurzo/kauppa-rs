//! MB-0056 P2 (RR-0038 sub-answer 4): a signed-out user signs in from the
//! 401 page, and lands on the page they asked for.

use std::time::Duration;

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};

use kauppa_core::domain::Language;

use super::{SIGNED_OUT, assert_page, login_form};
use crate::routes::test_support::{
    PASSWORD, body_string, login_request, make_user, session_cookie, test_app, test_state,
};

const EMAIL: &str = "u@example.com";

fn location<B>(resp: &actix_web::dev::ServiceResponse<B>) -> &str {
    resp.headers()
        .get(header::LOCATION)
        .expect("Location")
        .to_str()
        .expect("ascii")
}

/// The hidden `return` field in the sign-in form, if any.
fn return_field(form: &str) -> Option<&str> {
    let start = form.find(r#"name="return" value=""#)? + r#"name="return" value=""#.len();
    let end = start + form[start..].find('"')?;
    Some(&form[start..end])
}

/// A signed-out GET gets the 401 page with the sign-in form, which carries
/// the asked-for path and query; the status stays 401.
#[actix_web::test]
async fn form_in_the_page() {
    let state = test_state().await;
    let app = test_app!(state);
    for (lang, accept) in [(Language::En, "en"), (Language::Fi, "fi")] {
        let resp = call_service(
            &app,
            TestRequest::get()
                .uri("/recipes?sort=new")
                .insert_header((header::ACCEPT_LANGUAGE, accept))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let body = body_string(resp).await;
        assert_page(&state, &body, lang, SIGNED_OUT, accept);
        let form = login_form(&body).expect("sign-in form");
        assert!(form.contains(r#"method="post""#), "{form}");
        assert!(form.contains(r#"name="email""#), "{form}");
        assert!(form.contains(r#"name="password""#), "{form}");
        assert_eq!(return_field(form), Some("/recipes?sort&#x3D;new"), "{form}");
    }
}

/// No `return` where returning wouldn't show what the user was doing: after
/// a POST, and after an htmx request for part of a page. A boosted GET is a
/// page, so it keeps it.
#[actix_web::test]
async fn return_only_for_pages() {
    let state = test_state().await;
    let app = test_app!(state);
    let cases = [
        (
            "post",
            TestRequest::post().uri("/lists").set_form([("name", "x")]),
            None,
        ),
        (
            "fragment",
            TestRequest::get()
                .uri("/lists")
                .insert_header(("HX-Request", "true")),
            None,
        ),
        (
            "boosted",
            TestRequest::get()
                .uri("/lists")
                .insert_header(("HX-Request", "true"))
                .insert_header(("HX-Boosted", "true")),
            Some("/lists"),
        ),
    ];
    for (what, req, expected) in cases {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{what}");
        let body = body_string(resp).await;
        let form = login_form(&body).expect("sign-in form");
        assert_eq!(return_field(form), expected, "{what}: {form}");
    }
}

/// Signing in goes to `return` when it is a same-site path, else to `/`
/// (`same_site_path`); without `return`, to `/lists` as before.
#[actix_web::test]
async fn login_returns() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, EMAIL).await;
    let login = |ret: Option<&str>| {
        let mut form = vec![("email", EMAIL), ("password", PASSWORD)];
        if let Some(ret) = ret {
            form.push(("return", ret));
        }
        TestRequest::post().uri("/auth/login").set_form(form)
    };
    let cases = [
        (None, "/lists"),
        (Some(""), "/lists"),
        (Some("/recipes?sort=new"), "/recipes?sort=new"),
        (Some("//evil.example/"), "/"),
        (Some("/\\evil.example"), "/"),
        (Some("https://evil.example/"), "/"),
        (Some("%2F%2Fevil.example"), "/"),
        (Some("/\nLocation: https://evil.example"), "/"),
    ];
    for (ret, expected) in cases {
        let resp = call_service(&app, login(ret).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{ret:?}");
        assert_eq!(location(&resp), expected, "{ret:?}");
    }
}

/// A failed sign-in keeps `return` in the form it shows again.
#[actix_web::test]
async fn failed_login_keeps_return() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, EMAIL).await;
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/auth/login")
            .set_form([
                ("email", EMAIL),
                ("password", "not the password"),
                ("return", "/recipes"),
            ])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = body_string(resp).await;
    let form = login_form(&body).expect("sign-in form");
    assert_eq!(return_field(form), Some("/recipes"), "{form}");
}

/// A sign-in that ended (the idle deadline, RR-0026) no longer gives an
/// empty page: the 401 page, then back to the page.
#[actix_web::test]
async fn ended_sign_in() {
    // As `session_lifetime`: a 3 s idle deadline and a 4.5 s pause, with a
    // margin for whole-second timestamps and a jumpy wall clock.
    let state = test_state().await;
    make_user(&state, EMAIL).await;
    let app = test_app!(
        state,
        crate::identity_middleware(Duration::from_secs(3), Duration::from_secs(3600))
    );
    let session = session_cookie(&call_service(&app, login_request(EMAIL).to_request()).await);
    actix_web::rt::time::sleep(Duration::from_millis(4500)).await;
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/recipes")
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = body_string(resp).await;
    assert_page(&state, &body, Language::En, SIGNED_OUT, "ended");
    let form = login_form(&body).expect("sign-in form");
    assert_eq!(return_field(form), Some("/recipes"), "{form}");
}
