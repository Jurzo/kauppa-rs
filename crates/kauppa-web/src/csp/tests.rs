//! MB-0031 P1 and P3 (RR-0019): every response carries the security
//! headers, and they come from one place every route passes through.
#![allow(clippy::expect_used, clippy::panic)]

use actix_web::App;
use actix_web::dev::ServiceResponse;
use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service, init_service};

use super::{HSTS, POLICY, headers};
use crate::routes::assets::Assets;
use crate::routes::test_support::{
    bare_test_app, login_request, make_list, make_recipe, make_user, session_cookie, test_app,
    test_state,
};

const NIL: &str = "00000000-0000-0000-0000-000000000000";

/// The value of `name` on `res`, or `None`.
fn header_value<B>(res: &ServiceResponse<B>, name: &header::HeaderName) -> Option<String> {
    res.headers()
        .get(name)
        .map(|v| v.to_str().expect("ascii header").to_owned())
}

/// `res` carries the policy and the companion headers, with `referrer` as
/// its `Referrer-Policy` and no HSTS (the test app's cookies aren't
/// `Secure`).
fn assert_secured<B>(what: &str, res: &ServiceResponse<B>, referrer: &str) {
    assert_eq!(
        header_value(res, &header::CONTENT_SECURITY_POLICY).as_deref(),
        Some(POLICY),
        "{what}: {}",
        res.status()
    );
    assert_eq!(
        header_value(res, &header::X_CONTENT_TYPE_OPTIONS).as_deref(),
        Some("nosniff"),
        "{what}"
    );
    assert_eq!(
        header_value(res, &header::REFERRER_POLICY).as_deref(),
        Some(referrer),
        "{what}"
    );
    assert_eq!(
        header_value(res, &header::STRICT_TRANSPORT_SECURITY),
        None,
        "{what}"
    );
}

/// MB-0031 P1: full pages, fragments, error pages, the catch-all 404, the
/// cross-site 403 and `/assets` all carry the policy and the companion
/// headers; the register page keeps its own `Referrer-Policy:
/// no-referrer`; HSTS is sent only with `Secure` cookies.
#[actix_web::test]
async fn every_html_response_has_csp() {
    // The policy as RR-0019 sub-answer 1 writes it, word for word.
    assert_eq!(
        POLICY,
        "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; \
         object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"
    );
    for word in [
        "unsafe-", "nonce-", "sha256-", "sha384-", "sha512-", "data:", "http",
    ] {
        assert!(!POLICY.contains(word), "{word}");
    }

    let state = test_state().await;
    let user = make_user(&state, "csp@example.com").await;
    let list = make_list(&state, user, "Ruoka").await;
    let recipe = make_recipe(&state, user, "Puuro").await;
    let app = test_app!(state);

    // Signed out: full pages, redirects and error pages.
    for (uri, status) in [
        ("/", None),
        ("/healthz", Some(StatusCode::OK)),
        ("/auth/login", Some(StatusCode::OK)),
        ("/lists", None),
        ("/no-such-page", Some(StatusCode::NOT_FOUND)),
    ] {
        let res = call_service(&app, TestRequest::get().uri(uri).to_request()).await;
        if let Some(status) = status {
            assert_eq!(res.status(), status, "{uri}");
        }
        assert_secured(uri, &res, "same-origin");
    }

    // A failed login (401 page) and an extractor error (400).
    let res = call_service(
        &app,
        TestRequest::post()
            .uri("/auth/login")
            .set_form([("email", "csp@example.com"), ("password", "wrong")])
            .to_request(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert_secured("login 401", &res, "same-origin");
    let res = call_service(
        &app,
        TestRequest::post()
            .uri("/auth/login")
            .set_payload("not a form")
            .to_request(),
    )
    .await;
    assert!(res.status().is_client_error(), "{}", res.status());
    assert_secured("login bad form", &res, "same-origin");

    // The register page sets the stricter `no-referrer` itself (RR-0012);
    // the default doesn't overwrite it (RR-0019 constraint 5).
    let res = call_service(&app, TestRequest::get().uri("/auth/register").to_request()).await;
    assert_secured("register", &res, "no-referrer");

    // Signed in: one page per area, a 404 from a handler, and a fragment.
    let res = call_service(&app, login_request("csp@example.com").to_request()).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_secured("login 303", &res, "same-origin");
    let cookie = session_cookie(&res);
    let pages = [
        "/lists".to_owned(),
        format!("/lists/{list}"),
        format!("/lists/{list}/shop"),
        format!("/lists/{list}/sharing"),
        "/recipes".to_owned(),
        format!("/recipes/{recipe}"),
        "/favorites".to_owned(),
        "/categories".to_owned(),
        "/admin/invitations".to_owned(),
        format!("/lists/{NIL}"),
    ];
    for uri in &pages {
        let res = call_service(
            &app,
            TestRequest::get()
                .uri(uri)
                .cookie(cookie.clone())
                .to_request(),
        )
        .await;
        assert_secured(uri, &res, "same-origin");
    }
    let uri = format!("/recipes/{recipe}/ingredients/match?name=mai");
    let res = call_service(
        &app,
        TestRequest::get()
            .uri(&uri)
            .insert_header(("HX-Request", "true"))
            .cookie(cookie.clone())
            .to_request(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK, "{uri}");
    assert_secured("ingredient match fragment", &res, "same-origin");

    // The cross-site check's 403 (no proof at all).
    let bare = bare_test_app!(state);
    let res = call_service(
        &bare,
        TestRequest::post()
            .uri("/lists")
            .cookie(cookie)
            .set_form([("name", "x")])
            .to_request(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_secured("cross-site 403", &res, "same-origin");

    // `/assets`, which `main.rs` registers beside `routes::configure`, under
    // the same `.wrap`; and HSTS only with `Secure` cookies.
    for secure in [false, true] {
        let assets = init_service(App::new().wrap(headers(secure)).service(Assets)).await;
        for uri in ["/assets/app.css", "/assets/nope.css"] {
            let res = call_service(&assets, TestRequest::get().uri(uri).to_request()).await;
            assert_eq!(
                header_value(&res, &header::CONTENT_SECURITY_POLICY).as_deref(),
                Some(POLICY),
                "{uri}"
            );
            assert_eq!(
                header_value(&res, &header::X_CONTENT_TYPE_OPTIONS).as_deref(),
                Some("nosniff")
            );
            assert_eq!(
                header_value(&res, &header::STRICT_TRANSPORT_SECURITY),
                secure.then(|| HSTS.to_owned()),
                "{uri} secure={secure}"
            );
        }
    }
    assert_eq!(HSTS, "max-age=31536000");
}

/// The code of `text`, without `//` comment lines.
fn code(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .collect()
}

/// The `.wrap(` lines of the `App` builder in `text`, in order.
fn wraps(text: &str) -> Vec<&str> {
    code(text)
        .into_iter()
        .filter(|l| l.starts_with(".wrap("))
        .collect()
}

/// MB-0031 P3 (RR-0019 constraint 1): the headers come only from
/// `csp::headers`, wrapped last (outermost) in `main.rs`, with `main.rs`'s
/// cookie setting, and the same in `wired_app!`, which the route tests use.
/// Nothing else in the crate sets the policy or HSTS.
#[test]
fn headers_wrap_outermost() {
    let main = include_str!("../main.rs");
    let main_wraps = wraps(main);
    assert_eq!(
        main_wraps.last().copied(),
        Some(".wrap(csp::headers(cfg.cookie_secure))"),
        "main.rs: {main_wraps:?}"
    );
    assert_eq!(main_wraps.iter().filter(|l| l.contains("csp::")).count(), 1);

    let support = include_str!("../routes/test_support.rs");
    let start = support.find("macro_rules! wired_app").expect("wired_app!");
    let wired = &support[start..start + support[start..].find("\n}\n").expect("end")];
    let wired_wraps = wraps(wired);
    assert_eq!(
        wired_wraps.last().copied(),
        Some(".wrap(crate::csp::headers(false))"),
        "wired_app!: {wired_wraps:?}"
    );

    for (file, text) in crate::routes::test_support::source_files() {
        if file == "csp.rs" || file.starts_with("csp/") {
            continue;
        }
        for line in code(&text) {
            for name in [
                "CONTENT_SECURITY_POLICY",
                "Content-Security-Policy",
                "STRICT_TRANSPORT_SECURITY",
                "X_CONTENT_TYPE_OPTIONS",
                "DefaultHeaders",
            ] {
                assert!(
                    !line.contains(name),
                    "{file}: `{line}` sets a security header outside csp.rs"
                );
            }
        }
    }
}

/// MB-0046 P2 (RR-0029 sub-answer 2, constraint 3): every response that
/// doesn't set its own `Cache-Control` gets `no-store`, signed in or out;
/// the register page keeps its own; `/assets` sets its own. In the source,
/// only `csp.rs` and `routes/assets.rs` name a cache policy other than
/// `no-store`.
#[actix_web::test]
async fn signed_in_pages_are_not_cached() {
    let state = test_state().await;
    let app = test_app!(state);
    let user = make_user(&state, "cache@example.com").await;
    let list = make_list(&state, user, "Kauppa").await;
    let session = session_cookie(&call_service(&app, login_request("cache@example.com").to_request()).await);

    let cache = |res: &ServiceResponse<_>| header_value(res, &header::CACHE_CONTROL);
    for uri in [
        "/lists".to_owned(),
        format!("/lists/{list}"),
        "/recipes".to_owned(),
        "/favorites".to_owned(),
        "/categories".to_owned(),
        "/".to_owned(),
    ] {
        let res = call_service(&app, TestRequest::get().uri(&uri).cookie(session.clone()).to_request()).await;
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
        assert_eq!(cache(&res).as_deref(), Some("no-store"), "{uri}");
    }
    // A fragment, a signed-out page and a 404 too.
    let res = call_service(
        &app,
        TestRequest::post()
            .uri("/lists")
            .cookie(session.clone())
            .insert_header(("HX-Request", "true"))
            .set_form([("name", "Juhlat")])
            .to_request(),
    )
    .await;
    assert_eq!(cache(&res).as_deref(), Some("no-store"), "fragment");
    for uri in ["/auth/login", "/auth/register", "/nope"] {
        let res = call_service(&app, TestRequest::get().uri(uri).to_request()).await;
        assert_eq!(cache(&res).as_deref(), Some("no-store"), "{uri}");
    }
    // `/assets` answers with its own policy.
    let res = call_service(&app, TestRequest::get().uri("/assets/app.css").to_request()).await;
    assert_eq!(cache(&res).as_deref(), Some("no-cache"), "/assets/app.css");

    for (file, text) in crate::routes::test_support::source_files() {
        if file == "csp.rs" || file.starts_with("csp/") || file == "routes/assets.rs" {
            continue;
        }
        for line in code(&text) {
            if line.contains("CACHE_CONTROL") || line.contains("Cache-Control") {
                assert!(
                    line.contains("no-store") || !line.contains('"'),
                    "{file}: `{line}` sets a cache policy other than no-store"
                );
            }
        }
    }
}
