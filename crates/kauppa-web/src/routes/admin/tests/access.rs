//! MB-0023 P3: only administrators reach `/admin/*`. Anonymous requests go
//! to login, signed-in non-admins get the 404 page, and admin status is
//! read from the database on every request (RR-0012 constraint 5).

use std::str::FromStr;

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use chrono::Utc;
use sqlx::sqlite::SqliteConnectOptions;

use kauppa_core::domain::InvitationState;
use kauppa_core::repo::{AdminRepo, InvitationRepo};

use super::{ADMIN, MEMBER, admin_requests, make_admin};
use crate::routes::test_support::{
    body_string, login_request, make_user, session_cookie, session_kept, test_app, test_state,
    test_state_with_url,
};

/// What the app answers for a page that doesn't exist for this user.
/// Stands in for the request path in a compared page.
const PATH: &str = "PATH";

/// P3: anonymous requests to every admin route are sent to the login page
/// and change nothing.
#[actix_web::test]
async fn admin_only_anonymous_goes_to_login() {
    let state = test_state().await;
    let app = test_app!(state);
    let pending = InvitationRepo::create(&state.pool, None, None, Utc::now())
        .await
        .expect("invitation")
        .invitation
        .id;
    for req in admin_requests(pending, None) {
        let req = req.to_request();
        let what = format!("{} {}", req.method(), req.path());
        let resp = call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{what}");
        assert_eq!(
            resp.headers()
                .get(header::LOCATION)
                .map(actix_web::http::header::HeaderValue::as_bytes),
            Some(&b"/auth/login"[..]),
            "{what}"
        );
    }
    let listed = InvitationRepo::list(&state.pool).await.expect("list");
    assert_eq!(listed.len(), 1, "nothing created");
    assert_eq!(listed[0].state(Utc::now()), InvitationState::Pending);
}

/// P3: a signed-in non-admin gets the plain 404 on every admin route, the
/// same as for a list that isn't theirs, and changes nothing.
#[actix_web::test]
async fn admin_only_non_admin_gets_404() {
    let state = test_state().await;
    let app = test_app!(state);
    make_admin(&state, ADMIN).await;
    make_user(&state, MEMBER).await;
    let pending = InvitationRepo::create(&state.pool, None, None, Utc::now())
        .await
        .expect("invitation")
        .invitation
        .id;
    let session = session_cookie(&call_service(&app, login_request(MEMBER).to_request()).await);

    // The same 404 page as an unknown list; only the path the layout's
    // switches return to differs (MB-0056, RR-0038).
    let unknown_path = format!("/lists/{}", uuid::Uuid::new_v4());
    let unknown = call_service(
        &app,
        TestRequest::get()
            .uri(&unknown_path)
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    let not_found = body_string(unknown).await.replace(&unknown_path, PATH);
    assert!(not_found.contains("<main>"), "a page: {not_found}");

    for htmx in [false, true] {
        for req in admin_requests(pending, Some(&session)) {
            let req = if htmx {
                req.insert_header(("HX-Request", "true"))
            } else {
                req
            };
            let req = req.to_request();
            let path = req.path().to_owned();
            let what = format!("{} {path} htmx={htmx}", req.method());
            let resp = call_service(&app, req).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{what}");
            assert_eq!(
                body_string(resp).await.replace(&path, PATH),
                not_found,
                "{what}"
            );
        }
    }
    let listed = InvitationRepo::list(&state.pool).await.expect("list");
    assert_eq!(listed.len(), 1, "nothing created");
    assert_eq!(listed[0].state(Utc::now()), InvitationState::Pending);
}

/// P3: admin status comes from the database on every request: granting or
/// removing it takes effect on the same session's next request.
#[actix_web::test]
async fn admin_only_checked_on_every_request() {
    let (state, url) = test_state_with_url().await;
    let app = test_app!(state);
    let user = make_user(&state, MEMBER).await;
    let session = session_cookie(&call_service(&app, login_request(MEMBER).to_request()).await);
    let page = || {
        TestRequest::get()
            .uri("/admin/invitations")
            .cookie(session.clone())
            .to_request()
    };

    let resp = call_service(&app, page()).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "not yet");
    assert!(
        AdminRepo::grant(&state.pool, user, None, Utc::now())
            .await
            .expect("grant")
    );
    let resp = call_service(&app, page()).await;
    assert_eq!(resp.status(), StatusCode::OK, "granted: at once");
    // Admin status isn't put in the session: the cookie is kept (only the
    // visit-time renewal RR-0026 adds), and the next steps reuse the
    // original cookie.
    assert!(session_kept(&resp), "session unchanged");

    // Taking it away (no repo method does; a later brief) also takes effect
    // at once, with the same cookie.
    let raw = sqlx::SqlitePool::connect_with(SqliteConnectOptions::from_str(&url).expect("url"))
        .await
        .expect("raw pool");
    sqlx::query("DELETE FROM admins WHERE user_id = ?1")
        .bind(user.to_string())
        .execute(&raw)
        .await
        .expect("remove admin");
    let resp = call_service(&app, page()).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND, "removed: at once");

    // `/admin` itself is the invitations page, for admins only.
    AdminRepo::grant(&state.pool, user, None, Utc::now())
        .await
        .expect("grant");
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri("/admin")
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .map(actix_web::http::header::HeaderValue::as_bytes),
        Some(&b"/admin/invitations"[..])
    );
}
