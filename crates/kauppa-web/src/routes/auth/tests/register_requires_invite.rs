//! MB-0023 P1: registration needs a valid invitation, and every invalid one
//! gets the same answer (RR-0012 sub-answers 5 and 7, constraints 1 and 4).

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use chrono::{Duration, Utc};

use kauppa_core::domain::InvitationState;
use kauppa_core::repo::{InvitationRepo, UserRepo};

use super::{INVALID_INVITATION, hidden_invite, invitation, invitation_at, register_request};
use crate::routes::test_support::{
    LogCapture, body_string, make_user, session_cookie, source_files, test_app, test_state,
};

/// Both register responses are kept out of caches and referrers.
fn assert_private<B>(resp: &actix_web::dev::ServiceResponse<B>, what: &str) {
    let headers = resp.headers();
    assert_eq!(
        headers
            .get(header::REFERRER_POLICY)
            .map(actix_web::http::header::HeaderValue::as_bytes),
        Some(&b"no-referrer"[..]),
        "{what}: Referrer-Policy"
    );
    assert_eq!(
        headers
            .get(header::CACHE_CONTROL)
            .map(actix_web::http::header::HeaderValue::as_bytes),
        Some(&b"no-store"[..]),
        "{what}: Cache-Control"
    );
}

/// P1: missing, empty, malformed, unknown, expired, revoked and used tokens
/// all get the same 400 and the same body, and create nothing.
#[actix_web::test]
async fn register_requires_invite_same_answer_for_every_invalid_token() {
    let state = test_state().await;
    let app = test_app!(state);
    let now = Utc::now();

    let (_, expired) = invitation_at(&state, now - Duration::days(8)).await;
    let (revoked_id, revoked) = invitation_at(&state, now).await;
    InvitationRepo::revoke(&state.pool, revoked_id, now)
        .await
        .expect("revoke");
    let used = invitation(&state).await;
    let resp = call_service(
        &app,
        register_request(Some(&used), "first@example.com").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "the first use works");
    let unknown = "ab".repeat(32);

    let cases: [(&str, Option<&str>); 8] = [
        ("missing", None),
        ("empty", Some("")),
        ("malformed", Some("not-a-token")),
        ("upper case", Some(&"AB".repeat(32))),
        ("unknown", Some(&unknown)),
        ("expired", Some(&expired)),
        ("revoked", Some(&revoked)),
        ("used", Some(&used)),
    ];
    // Each with a new email, and with one that already has an account: an
    // invalid token never reveals which emails are taken.
    make_user(&state, "taken@example.com").await;
    let mut bodies = Vec::new();
    for (label, token) in cases
        .iter()
        .flat_map(|&case| [(case, "pat@example.com"), (case, "taken@example.com")])
        .map(|((label, token), email)| (format!("{label} ({email})"), (token, email)))
    {
        let (token, email) = token;
        let label = label.as_str();
        let resp = call_service(&app, register_request(token, email).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{label}");
        assert_private(&resp, label);
        assert_eq!(resp.response().cookies().count(), 0, "{label}: no session");
        let body = body_string(resp).await;
        assert!(body.contains(INVALID_INVITATION), "{label}: {body}");
        assert!(
            UserRepo::find_by_email(&state.pool, "pat@example.com")
                .await
                .expect("lookup")
                .is_none(),
            "{label}: no account"
        );
        bodies.push((label.to_owned(), body));
    }
    for (label, body) in &bodies[1..] {
        assert_eq!(*body, bodies[0].1, "{label}: same body as missing");
    }
    // The invalid answer offers no form to retry with.
    assert!(!bodies[0].1.contains("type=\"password\""));

    // The revoked and expired invitations are still what they were.
    let states: Vec<_> = InvitationRepo::list(&state.pool)
        .await
        .expect("list")
        .iter()
        .map(|i| i.state(now))
        .collect();
    assert_eq!(states.len(), 3);
    assert_eq!(
        states
            .iter()
            .filter(|s| **s == InvitationState::Used)
            .count(),
        1
    );
}

/// P1: the invitation link renders the form with the token as a hidden
/// field, posting to `/auth/register` without a query; posting it creates
/// the account once and signs it in.
#[actix_web::test]
async fn register_requires_invite_valid_token_creates_one_account() {
    let state = test_state().await;
    let app = test_app!(state);
    let token = invitation(&state).await;

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/auth/register?invite={token}"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_private(&resp, "GET with invite");
    let page = body_string(resp).await;
    assert_eq!(hidden_invite(&page).as_deref(), Some(token.as_str()));
    assert!(
        page.contains("<form method=\"post\" action=\"/auth/register\">"),
        "{page}"
    );

    let resp = call_service(
        &app,
        register_request(Some(&token), "pat@example.com").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_private(&resp, "POST success");
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .map(actix_web::http::header::HeaderValue::as_bytes),
        Some(&b"/lists"[..])
    );
    let session = session_cookie(&resp);
    let lists = call_service(
        &app,
        TestRequest::get()
            .uri("/lists")
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(lists.status(), StatusCode::OK, "signed in");

    let (user, _) = UserRepo::find_by_email(&state.pool, "pat@example.com")
        .await
        .expect("lookup")
        .expect("account created");
    let listed = InvitationRepo::list(&state.pool).await.expect("list");
    assert_eq!(listed[0].state(Utc::now()), InvitationState::Used);
    assert_eq!(listed[0].used_by.as_ref().map(|u| u.id), Some(user.id));

    // Once only.
    let resp = call_service(
        &app,
        register_request(Some(&token), "sam@example.com").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert!(
        UserRepo::find_by_email(&state.pool, "sam@example.com")
            .await
            .expect("lookup")
            .is_none()
    );
}

/// P1: without a token the register page says registration needs an
/// invitation and has no form; neither the login nor the home page links
/// to it.
#[actix_web::test]
async fn register_requires_invite_page_without_token() {
    let state = test_state().await;
    let app = test_app!(state);
    for uri in ["/auth/register", "/auth/register?invite="] {
        let resp = call_service(&app, TestRequest::get().uri(uri).to_request()).await;
        assert_eq!(resp.status(), StatusCode::OK, "{uri}");
        assert_private(&resp, uri);
        let page = body_string(resp).await;
        assert!(page.contains("needs an invitation"), "{uri}: {page}");
        assert!(!page.contains("type=\"password\""), "{uri}: no form");
        assert!(hidden_invite(&page).is_none(), "{uri}");
    }
    for uri in ["/auth/login", "/"] {
        let page =
            body_string(call_service(&app, TestRequest::get().uri(uri).to_request()).await).await;
        assert!(!page.contains("/auth/register"), "{uri} links to sign-up");
    }
}

/// RR-0012 sub-answer 4: a duplicate email is a form error with the
/// invitation kept, and the email never reaches the log.
#[actix_web::test]
async fn register_requires_invite_duplicate_email_keeps_the_form() {
    // As in `error::tests`: a second, idle dispatcher keeps parallel tests
    // from switching callsites off; the rebuild recomputes interest.
    let _idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let capture = LogCapture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "taken.person@example.com").await;
    let token = invitation(&state).await;

    let resp = call_service(
        &app,
        register_request(Some(&token), "taken.person@example.com").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    assert_private(&resp, "duplicate email");
    let page = body_string(resp).await;
    assert!(page.contains("already"), "{page}");
    assert_eq!(hidden_invite(&page).as_deref(), Some(token.as_str()));
    assert!(page.contains("type=\"password\""), "the form again");
    assert_eq!(
        InvitationRepo::list(&state.pool).await.expect("list")[0].state(Utc::now()),
        InvitationState::Pending
    );

    let resp = call_service(
        &app,
        register_request(Some(&token), "pat@example.com").to_request(),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "the invitation still works"
    );

    let log = capture.text();
    assert!(!log.contains("taken.person"), "{log}");
}

/// RR-0012 constraint 1: outside tests, accounts are created only by
/// `InvitationRepo::register`. No non-test source in this crate calls
/// `UserRepo::create`: not by path, and no file that imports `UserRepo`
/// calls a `.create(` method (other repos' `create` by path is fine).
#[test]
fn register_requires_invite_no_bypass() {
    let files = source_files();
    assert!(
        files.iter().any(|(f, _)| f == "routes/auth/mod.rs"),
        "scan works"
    );
    for (file, text) in files {
        assert!(
            !text.contains("UserRepo::create"),
            "{file} calls UserRepo::create"
        );
        assert!(
            !text.contains("repo::*"),
            "{file} glob-imports the repo traits"
        );
        if text.contains("UserRepo") {
            assert!(
                !text.contains(".create("),
                "{file} imports UserRepo and calls a create method"
            );
        }
    }
}
