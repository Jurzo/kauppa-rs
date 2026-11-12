//! MB-0018 P1 and P3: forged and missing proof is rejected on every write
//! route, as a plain form post and over htmx, with nothing changed. These
//! use `bare_test_app!`: requests carry exactly the headers built here.

use actix_web::cookie::Cookie;
use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};

use super::{World, world, write_routes};
use crate::routes::test_support::{
    TEST_HOST, TEST_ORIGIN, bare_test_app, body_string, login_request, session_cookie,
    session_kept, test_state,
};

/// Header sets a forged request might carry (besides `Host`).
type Proof = (&'static str, &'static [(&'static str, &'static str)]);

/// Cross-site and same-site requests (P1).
const FORGED: &[Proof] = &[
    ("foreign origin", &[("Origin", "http://evil.example")]),
    (
        "sibling subdomain",
        &[("Origin", "http://shop.localhost:8080")],
    ),
    ("other scheme", &[("Origin", "https://localhost:8080")]),
    ("other port", &[("Origin", "http://localhost:8081")]),
    (
        "foreign referer",
        &[("Referer", "http://evil.example/lists")],
    ),
    (
        "foreign origin, own referer",
        &[
            ("Origin", "http://evil.example"),
            ("Referer", "http://localhost:8080/lists"),
        ],
    ),
    (
        "fetch metadata cross-site",
        &[("Sec-Fetch-Site", "cross-site"), ("Origin", TEST_ORIGIN)],
    ),
    (
        "fetch metadata same-site",
        &[("Sec-Fetch-Site", "same-site"), ("Origin", TEST_ORIGIN)],
    ),
];

/// Requests without usable proof (P3).
const MISSING: &[Proof] = &[
    ("no headers", &[]),
    ("null origin", &[("Origin", "null")]),
    ("empty origin", &[("Origin", "")]),
    ("unparsable origin", &[("Origin", "localhost:8080")]),
    ("relative referer", &[("Referer", "/lists")]),
    ("fetch metadata alone", &[("Sec-Fetch-Site", "same-origin")]),
    ("fetch metadata none alone", &[("Sec-Fetch-Site", "none")]),
];

/// Sends every write route each proof in `proofs`, signed in, as a plain
/// post and over htmx; each must get the plain 403, keep the session (only
/// the visit-time renewal RR-0026 adds) and change nothing. Returns how many requests were sent.
async fn assert_all_rejected(proofs: &[Proof]) -> usize {
    let state = test_state().await;
    let app = bare_test_app!(state);
    let w: World = world(&state).await;
    let login = login_request(super::OWNER)
        .insert_header((header::HOST, TEST_HOST))
        .insert_header((header::ORIGIN, TEST_ORIGIN));
    let session: Cookie<'static> = session_cookie(&call_service(&app, login.to_request()).await);
    let before = w.snapshot(&state).await;

    let mut sent = 0;
    for route in write_routes() {
        for &(label, headers) in proofs {
            for htmx in [false, true] {
                let mut req = TestRequest::default()
                    .method(route.method.clone())
                    .uri(&w.uri(&route))
                    .insert_header((header::HOST, TEST_HOST))
                    .cookie(session.clone())
                    .set_form(w.form(&route));
                for &(name, value) in headers {
                    req = req.insert_header((name, value));
                }
                if htmx {
                    req = req.insert_header(("HX-Request", "true"));
                }
                let resp = call_service(&app, req.to_request()).await;
                let what = format!("{} {} ({label}, htmx={htmx})", route.method, route.path);
                assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{what}");
                assert!(
                    session_kept(&resp),
                    "{what}: no session issued or purged (RR-0026 renews its visit time)"
                );
                assert_eq!(body_string(resp).await, "forbidden", "{what}: plain 403");
                sent += 1;
            }
        }
    }
    assert_eq!(w.snapshot(&state).await, before, "nothing changed");

    // The victim's session survived the forged logouts.
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/lists")
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK, "still signed in");
    sent
}

/// MB-0018 P1: a cross-site (or same-site) request to any write route gets
/// 403 and changes nothing.
#[actix_web::test]
async fn rejects_cross_site_writes() {
    let sent = assert_all_rejected(FORGED).await;
    assert!(sent >= 34 * FORGED.len() * 2, "{sent}");
}

/// MB-0018 P3: stripping or spoiling the proof isn't a bypass; Fetch
/// Metadata alone never admits a request. Unregistered paths are covered
/// too: the check runs before routing.
#[actix_web::test]
async fn missing_proof_is_rejected() {
    let sent = assert_all_rejected(MISSING).await;
    assert!(sent >= 34 * MISSING.len() * 2, "{sent}");

    let state = test_state().await;
    let app = bare_test_app!(state);
    for method in ["POST", "PUT", "PATCH", "DELETE"] {
        let req = TestRequest::default()
            .method(method.parse().expect("method"))
            .uri("/no/such/route")
            .insert_header((header::HOST, TEST_HOST))
            .to_request();
        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{method}");
    }
    // A request with proof reaches routing as before.
    let req = TestRequest::post()
        .uri("/no/such/route")
        .insert_header((header::HOST, TEST_HOST))
        .insert_header((header::ORIGIN, TEST_ORIGIN))
        .to_request();
    assert_eq!(
        call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

/// MB-0023: the forged requests above are refused by the check, not by the
/// handlers: the same form, sent same-origin by the signed-in admin, does
/// create an invitation, revoke one and create an account.
#[actix_web::test]
async fn invite_routes_reject_only_on_proof() {
    let state = test_state().await;
    let app = bare_test_app!(state);
    let w: World = world(&state).await;
    let login = login_request(super::OWNER)
        .insert_header((header::HOST, TEST_HOST))
        .insert_header((header::ORIGIN, TEST_ORIGIN));
    let session: Cookie<'static> = session_cookie(&call_service(&app, login.to_request()).await);
    for path in [
        "/admin/invitations",
        "/admin/invitations/{id}/revoke",
        "/auth/register",
    ] {
        let route = write_routes()
            .into_iter()
            .find(|r| r.path == path)
            .expect("route");
        // The revoke above revoked the world's invitation: registration
        // gets a fresh one.
        let w = if path == "/auth/register" {
            world_invite_only(&state, &w).await
        } else {
            w.clone()
        };
        let before = w.snapshot(&state).await;
        let req = TestRequest::post()
            .uri(&w.uri(&route))
            .insert_header((header::HOST, TEST_HOST))
            .insert_header((header::ORIGIN, TEST_ORIGIN))
            .cookie(session.clone())
            .set_form(w.form(&route));
        let status = call_service(&app, req.to_request()).await.status();
        assert!(
            status.is_success() || status.is_redirection(),
            "{path}: {status}"
        );
        assert_ne!(w.snapshot(&state).await, before, "{path} changed something");
    }
}

/// `w` with a new pending invitation in place of its own.
async fn world_invite_only(state: &crate::state::AppState, w: &World) -> World {
    let created = kauppa_core::repo::InvitationRepo::create(
        &state.pool,
        Some(w.owner),
        None,
        chrono::Utc::now(),
    )
    .await
    .expect("invitation");
    World {
        invitation: created.invitation.id,
        invite: created.token.as_str().to_owned(),
        ..w.clone()
    }
}
