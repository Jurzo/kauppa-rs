//! MB-0056 P4 (RR-0038 constraints 1 and 2): an error page discloses
//! nothing new. It never says which check refused a request, a hidden list
//! looks like a missing one, and no error detail or input reaches the page.

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use uuid::Uuid;

use kauppa_core::domain::ShareRole;
use kauppa_core::repo::ShoppingListRepo;

use super::main_of;
use crate::routes::test_support::{
    bare_test_app, body_string, login_request, make_list, make_user, session_cookie, test_app,
    test_state,
};

/// The cross-site check's 403 and an authorization 403 are the same bytes
/// for the same request (`csrf/mod.rs`: the client can't tell which check
/// failed).
#[actix_web::test]
async fn forbidden_pages_identical() {
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
    let rename = || {
        TestRequest::post()
            .uri(&format!("/lists/{list}/rename"))
            .cookie(viewer.clone())
            .set_form([("name", "Uusi")])
    };
    for hx in [false, true] {
        let with_hx = |req: TestRequest| {
            if hx {
                req.insert_header(("HX-Request", "true"))
            } else {
                req
            }
        };
        // Authorization: a same-origin post by a view grantee.
        let resp = call_service(&app, with_hx(rename()).to_request()).await;
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "authorization hx={hx}"
        );
        let authorization = body_string(resp).await;
        // The cross-site check: the same post from another origin.
        let resp = call_service(
            &bare,
            with_hx(rename())
                .insert_header((header::ORIGIN, "http://evil.example"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "cross-site hx={hx}");
        let cross_site = body_string(resp).await;
        assert_eq!(authorization, cross_site, "hx={hx}");
    }
}

/// A list the user may not see is the same page as one that doesn't exist
/// (RR-0002 constraint 4); only the path the layout's switches return to
/// differs.
#[actix_web::test]
async fn hidden_list_like_missing() {
    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    make_user(&state, "stranger@example.com").await;
    let list = make_list(&state, owner, "Kauppa").await;
    let stranger = session_cookie(
        &call_service(&app, login_request("stranger@example.com").to_request()).await,
    );
    let page = |path: String| {
        let cookie = stranger.clone();
        let app = &app;
        async move {
            let resp = call_service(
                app,
                TestRequest::get().uri(&path).cookie(cookie).to_request(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{path}");
            body_string(resp).await.replace(&path, "PATH")
        }
    };
    let hidden = page(format!("/lists/{list}")).await;
    let missing = page(format!("/lists/{}", Uuid::new_v4())).await;
    assert_eq!(hidden, missing);
}

/// What the request carried, and what the error was, never reach what the
/// page says.
#[actix_web::test]
async fn no_input_or_detail() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "u@example.com").await;
    let s = session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let marker = "Zq7Marker";
    let cases = [
        (
            "rejected value",
            TestRequest::post().uri("/theme").set_form([
                ("theme", format!("<b>{marker}</b>")),
                ("return", "/".to_owned()),
            ]),
            StatusCode::BAD_REQUEST,
        ),
        (
            "malformed id",
            TestRequest::get()
                .uri(&format!("/lists/{marker}"))
                .cookie(s.clone()),
            StatusCode::NOT_FOUND,
        ),
        (
            "malformed form",
            TestRequest::post()
                .uri("/lists")
                .cookie(s.clone())
                .set_form([(marker, marker)]),
            StatusCode::BAD_REQUEST,
        ),
    ];
    let leaks = [
        marker,
        "Invalid",
        "Theme",
        "deserialize",
        "Parse error",
        "missing field",
        "UUID",
    ];
    for (what, req, status) in cases {
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), status, "{what}");
        let body = body_string(resp).await;
        let main = main_of(&body);
        for leak in leaks {
            assert!(!main.contains(leak), "{what}: {leak:?} on the page\n{main}");
        }
        assert!(
            !body.contains(&format!("<b>{marker}")),
            "{what}: input unescaped"
        );
    }
}
