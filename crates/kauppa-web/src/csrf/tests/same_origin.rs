//! MB-0018 P2: same-origin form posts and htmx requests still succeed,
//! including login and registration, directly (same-host) and behind a
//! proxy with `KAUPPA_PUBLIC_ORIGIN`. `bare_test_app!`: every request
//! carries exactly the proof a browser would.

use actix_web::cookie::Cookie;
use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use actix_web::web;

use kauppa_core::repo::{ShoppingListRepo, UserRepo};

use crate::csrf::{AppOrigin, OriginPolicy};
use crate::routes::test_support::{
    PASSWORD, TEST_HOST, TEST_ORIGIN, bare_test_app, body_string, session_cookie, test_state,
};
use crate::state::AppState;

/// The `action` of the first `<form method="post">` in `page` whose action
/// starts with `prefix`, as a browser would submit it.
fn form_action(page: &str, prefix: &str) -> String {
    page.split("<form")
        .skip(1)
        .filter(|form| form.contains("method=\"post\""))
        .filter_map(|form| form.split("action=\"").nth(1)?.split('"').next())
        .find(|action| action.starts_with(prefix))
        .unwrap_or_else(|| panic!("no post form for {prefix} in {page}"))
        .to_owned()
}

/// What a browser on `origin` adds to a request to `host`: `Host`, plus
/// `Origin` (or only `Referer`, when `referer_only`), plus `HX-Request` for
/// htmx and `Sec-Fetch-Site: same-origin`.
fn from_page(
    req: TestRequest,
    host: &str,
    origin: &str,
    htmx: bool,
    referer_only: bool,
) -> TestRequest {
    let mut req = req.insert_header((header::HOST, host.to_owned()));
    req = if referer_only {
        req.insert_header((header::REFERER, format!("{origin}/lists")))
    } else {
        req.insert_header((header::ORIGIN, origin.to_owned()))
    };
    if htmx {
        req = req
            .insert_header(("HX-Request", "true"))
            .insert_header(("Sec-Fetch-Site", "same-origin"));
    }
    req
}

/// Registers, signs in, creates lists (plain, htmx, Referer-only) and signs
/// out, each through the form the app renders, with `host` and `origin` as
/// a browser sends them. Every step must succeed.
async fn browse(state: &web::Data<AppState>, host: &str, origin: &str) {
    let app = bare_test_app!(state);
    let get = |uri: &str| {
        TestRequest::get()
            .uri(uri)
            .insert_header((header::HOST, host.to_owned()))
    };

    // Registration and login, as plain posts and over htmx (hx-boost).
    for (htmx, email) in [(false, "plain@example.com"), (true, "htmx@example.com")] {
        let page = body_string(call_service(&app, get("/auth/register").to_request()).await).await;
        let action = form_action(&page, "/auth/register");
        let form = [
            ("email", email),
            ("display_name", "Pat"),
            ("password", PASSWORD),
        ];
        let req = from_page(TestRequest::post().uri(&action), host, origin, htmx, false);
        let resp = call_service(&app, req.set_form(form).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "register htmx={htmx}");
        assert!(
            UserRepo::find_by_email(&state.pool, email)
                .await
                .expect("lookup")
                .is_some()
        );

        let page = body_string(call_service(&app, get("/auth/login").to_request()).await).await;
        let action = form_action(&page, "/auth/login");
        let req = from_page(TestRequest::post().uri(&action), host, origin, htmx, false);
        let resp = call_service(
            &app,
            req.set_form([("email", email), ("password", PASSWORD)])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "login htmx={htmx}");
        let session: Cookie<'static> = session_cookie(&resp);

        // Creating a list from the lists page: plain, htmx, Referer only.
        let page = body_string(
            call_service(&app, get("/lists").cookie(session.clone()).to_request()).await,
        )
        .await;
        let action = form_action(&page, "/lists");
        for (n, (list_htmx, referer_only)) in [(false, false), (true, false), (false, true)]
            .into_iter()
            .enumerate()
        {
            let req = from_page(
                TestRequest::post().uri(&action).cookie(session.clone()),
                host,
                origin,
                list_htmx,
                referer_only,
            );
            let name = format!("Lista {n}");
            let resp =
                call_service(&app, req.set_form([("name", name.as_str())]).to_request()).await;
            assert!(
                resp.status().is_success() || resp.status().is_redirection(),
                "create list htmx={list_htmx} referer_only={referer_only}: {}",
                resp.status()
            );
        }
        let user = UserRepo::find_by_email(&state.pool, email)
            .await
            .expect("lookup")
            .expect("user")
            .0
            .id;
        let lists = ShoppingListRepo::list_for_owner(&state.pool, user)
            .await
            .expect("lists");
        assert_eq!(lists.len(), 3, "htmx={htmx}");

        // Signing out from the layout's form.
        let action = form_action(&page, "/auth/logout");
        let req = from_page(
            TestRequest::post().uri(&action).cookie(session),
            host,
            origin,
            htmx,
            false,
        );
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "logout htmx={htmx}");
    }
}

/// MB-0018 P2: same-origin writes succeed on a direct connection (same-host
/// policy) and behind a reverse proxy that rewrites `Host`, once
/// `KAUPPA_PUBLIC_ORIGIN` names the public origin.
#[actix_web::test]
async fn same_origin_writes_succeed() {
    // Direct: the app's origin is the request's own Host.
    browse(&test_state().await, TEST_HOST, TEST_ORIGIN).await;
    browse(
        &test_state().await,
        "192.168.1.10:8080",
        "http://192.168.1.10:8080",
    )
    .await;

    // Behind a proxy that rewrites Host to the upstream address.
    let base = test_state().await;
    let public = AppOrigin::parse("https://kauppa.example").expect("origin");
    let state = web::Data::new(AppState {
        origin: OriginPolicy::Fixed(public),
        ..(*base.into_inner()).clone()
    });
    browse(&state, "127.0.0.1:8080", "https://kauppa.example").await;

    // There, a request whose Origin merely matches the rewritten Host is
    // cross-site: the browser was never on that origin.
    let app = bare_test_app!(state);
    let req = TestRequest::post()
        .uri("/auth/login")
        .insert_header((header::HOST, "127.0.0.1:8080"))
        .insert_header((header::ORIGIN, "http://127.0.0.1:8080"))
        .set_form([("email", "plain@example.com"), ("password", PASSWORD)]);
    let resp = call_service(&app, req.to_request()).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
