//! MB-0056 P3 (RR-0038 sub-answer 5): under htmx an error page replaces
//! `<body>`, boosted or not; nothing else gets the swap headers.

use actix_web::http::StatusCode;
use actix_web::test::{TestRequest, call_service};
use uuid::Uuid;

use kauppa_core::domain::Language;

use super::{NOT_FOUND, assert_page, header_of};
use crate::routes::test_support::{
    body_string, login_request, make_user, session_cookie, test_app, test_state,
};

#[actix_web::test]
async fn swaps_body() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "u@example.com").await;
    let s = session_cookie(&call_service(&app, login_request("u@example.com").to_request()).await);
    let missing = format!("/lists/{}", Uuid::new_v4());

    let cases = [
        ("plain", false, false, None),
        ("boosted", true, true, Some("body")),
        ("fragment", true, false, Some("body")),
    ];
    for (what, hx, boosted, retarget) in cases {
        let mut req = TestRequest::get().uri(&missing).cookie(s.clone());
        if hx {
            req = req.insert_header(("HX-Request", "true"));
        }
        if boosted {
            req = req.insert_header(("HX-Boosted", "true"));
        }
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{what}");
        assert_eq!(
            header_of(&resp, "hx-retarget").as_deref(),
            retarget,
            "{what}"
        );
        assert_eq!(
            header_of(&resp, "hx-reswap").as_deref(),
            retarget.map(|_| "innerHTML"),
            "{what}"
        );
        // A whole page, never a fragment for the request's own target.
        let body = body_string(resp).await;
        assert_page(&state, &body, Language::En, NOT_FOUND, what);
    }

    // A fragment write that fails the same way (the list is gone).
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("{missing}/items"))
            .cookie(s.clone())
            .insert_header(("HX-Request", "true"))
            .set_form([("text", "Maito")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(header_of(&resp, "hx-retarget").as_deref(), Some("body"));
}

/// A handler's own 4xx page (the login form's, RR-0025) gets no swap
/// headers: its form's `hx-target-4xx` does the swapping, as before.
#[actix_web::test]
async fn handler_pages_keep_their_swap() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "u@example.com").await;
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/auth/login")
            .insert_header(("HX-Request", "true"))
            .insert_header(("HX-Boosted", "true"))
            .set_form([("email", "u@example.com"), ("password", "wrong password!")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(header_of(&resp, "hx-retarget"), None);
    assert_eq!(header_of(&resp, "hx-reswap"), None);
}
