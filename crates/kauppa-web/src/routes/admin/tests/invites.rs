//! MB-0023 P4 and P7: creating, listing and revoking invitations in the
//! portal, and the audit log lines.

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use actix_web::web;
use chrono::{Duration, Utc};

use kauppa_core::repo::InvitationRepo;
use kauppa_core::token;

use super::{ADMIN, invitation_link, make_admin};
use crate::csrf::{AppOrigin, OriginPolicy};
use crate::routes::auth::tests::register_request;
use crate::routes::test_support::{
    body_string, login_request, session_cookie, test_app, test_state,
};
use crate::state::AppState;

fn header_is<B>(resp: &actix_web::dev::ServiceResponse<B>, name: &header::HeaderName, value: &str) {
    assert_eq!(
        resp.headers().get(name).and_then(|v| v.to_str().ok()),
        Some(value),
        "{name}"
    );
}

/// P4: create an invitation with a note and see its link once; list them
/// with their state; revoke a pending one. Plain forms throughout, so the
/// portal works with htmx disabled.
#[actix_web::test]
async fn create_list_revoke() {
    let state = test_state().await;
    let app = test_app!(state);
    make_admin(&state, ADMIN).await;
    let session = session_cookie(&call_service(&app, login_request(ADMIN).to_request()).await);
    let get_page = || async {
        let resp = call_service(
            &app,
            TestRequest::get()
                .uri("/admin/invitations")
                .cookie(session.clone())
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        body_string(resp).await
    };
    let create = |note: &str| {
        TestRequest::post()
            .uri("/admin/invitations")
            .cookie(session.clone())
            .set_form([("note", note)])
            .to_request()
    };

    // An empty portal, with a plain form to create one.
    let page = get_page().await;
    assert!(page.contains("No invitations yet"), "{page}");
    assert!(
        page.contains("<form class=\"add-product\" method=\"post\" action=\"/admin/invitations\">"),
        "{page}"
    );
    assert!(page.contains("<label>Note"), "labelled field");

    // Create: the link is shown once, absolute for the request's origin.
    let resp = call_service(&app, create("for Mum")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    header_is(&resp, &header::CACHE_CONTROL, "no-store");
    header_is(&resp, &header::REFERRER_POLICY, "no-referrer");
    let page = body_string(resp).await;
    let (link, mum_token) = invitation_link(&page).expect("link shown");
    assert!(
        link.starts_with("http://localhost:8080/auth/register?invite="),
        "{link}"
    );
    assert!(token::is_well_formed(&mum_token), "{mum_token}");
    assert!(page.contains("for Mum"));
    let listed = InvitationRepo::list(&state.pool).await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].note.as_deref(), Some("for Mum"));

    // The list shows it as pending, never its link again.
    let page = get_page().await;
    assert!(!page.contains(&mum_token), "shown once");
    assert!(
        page.contains("for Mum") && page.contains("· pending"),
        "{page}"
    );
    let mum_revoke = format!("/admin/invitations/{}/revoke", listed[0].id);
    assert!(page.contains(&mum_revoke), "a pending one can be revoked");

    // Used: by whom.
    let resp = call_service(
        &app,
        register_request(Some(&mum_token), "mum@example.com").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let page = get_page().await;
    assert!(
        page.contains("· used") && page.contains("mum@example.com"),
        "{page}"
    );
    assert!(!page.contains(&mum_revoke), "a used one can't be revoked");

    // A second one, without a note, revoked as a plain form post.
    let resp = call_service(&app, create("  ")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let (_, token2) = invitation_link(&body_string(resp).await).expect("link");
    // (Both were created within a second: find it by state, not order.)
    let pool = &state.pool;
    let by_use = |used: bool| async move {
        InvitationRepo::list(pool)
            .await
            .expect("list")
            .into_iter()
            .find(|i| i.used_at.is_some() == used)
            .expect("invitation")
            .id
    };
    let second = by_use(false).await;
    let revoke = |id: String| {
        TestRequest::post()
            .uri(&format!("/admin/invitations/{id}/revoke"))
            .cookie(session.clone())
            .to_request()
    };
    let resp = call_service(&app, revoke(second.to_string())).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    header_is(&resp, &header::LOCATION, "/admin/invitations");
    let page = get_page().await;
    assert!(page.contains("· revoked"), "{page}");
    let resp = call_service(
        &app,
        register_request(Some(&token2), "late@example.com").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "revoked link");

    // Revoking anything but a pending invitation: 404.
    let used = by_use(true).await;
    for id in [
        second.to_string(),
        used.to_string(),
        uuid::Uuid::new_v4().to_string(),
        "not-a-uuid".to_owned(),
    ] {
        let resp = call_service(&app, revoke(id.clone())).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{id}");
    }

    // Expired: listed as such, no revoke button.
    let expired = InvitationRepo::create(&state.pool, None, None, Utc::now() - Duration::days(8))
        .await
        .expect("old invitation")
        .invitation
        .id;
    let page = get_page().await;
    assert!(page.contains("· expired"), "{page}");
    assert!(!page.contains(&format!("/admin/invitations/{expired}/revoke")));

    // A note that's too long is a form error; nothing is created.
    let resp = call_service(&app, create(&"x".repeat(201))).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let page = body_string(resp).await;
    assert!(page.contains("at most 200"), "{page}");
    assert!(invitation_link(&page).is_none());
    assert_eq!(
        InvitationRepo::list(&state.pool).await.expect("list").len(),
        3
    );
}

/// P4: over htmx (`hx-boost`), creating returns the whole page with the
/// link; the link uses `KAUPPA_PUBLIC_ORIGIN` when set.
#[actix_web::test]
async fn create_list_revoke_boosted_with_public_origin() {
    let base = test_state().await;
    let state = web::Data::new(AppState {
        origin: OriginPolicy::Fixed(AppOrigin::parse("https://kauppa.example").expect("origin")),
        ..(*base.into_inner()).clone()
    });
    let app = test_app!(state);
    make_admin(&state, ADMIN).await;
    let login = login_request(ADMIN).insert_header((header::ORIGIN, "https://kauppa.example"));
    let session = session_cookie(&call_service(&app, login.to_request()).await);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/admin/invitations")
            .insert_header((header::ORIGIN, "https://kauppa.example"))
            .insert_header(("HX-Request", "true"))
            .insert_header(("HX-Boosted", "true"))
            .cookie(session)
            .set_form([("note", "")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let page = body_string(resp).await;
    assert!(page.contains("<html"), "the full page");
    let (link, _) = invitation_link(&page).expect("link");
    assert!(
        link.starts_with("https://kauppa.example/auth/register?invite="),
        "{link}"
    );
}
