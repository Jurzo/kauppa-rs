//! MB-0044 (RR-0028): email addresses that differ only by ASCII case, or by
//! surrounding whitespace, are one address for sign-in, registration, the
//! login limiter and sharing.

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};

use kauppa_core::domain::ListAccess;
use kauppa_core::repo::{ShoppingListRepo, UserRepo};

use super::{invitation, register_request};
use crate::routes::auth::limiter::LOGIN_PER_ACCOUNT;
use crate::routes::test_support::{
    PASSWORD, body_string, login_request, make_list, make_user, session_cookie, test_app,
    test_state,
};

fn addr(ip: &str) -> SocketAddr {
    SocketAddr::new(IpAddr::from_str(ip).expect("ip"), 50_000)
}

fn location<B>(resp: &actix_web::dev::ServiceResponse<B>) -> Option<&str> {
    resp.headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
}

/// P1: sign-in finds the account whatever the case, and with surrounding
/// whitespace; a wrong password and an unknown email still get the same
/// answer.
#[actix_web::test]
async fn login_ignores_case() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "Pat@Example.com").await;

    for typed in [
        "Pat@Example.com",
        "pat@example.com",
        "PAT@EXAMPLE.COM",
        "  pat@EXAMPLE.com  ",
    ] {
        let resp = call_service(&app, login_request(typed).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{typed:?}");
        assert_eq!(location(&resp), Some("/lists"), "{typed:?}");
    }

    let wrong = call_service(
        &app,
        TestRequest::post()
            .uri("/auth/login")
            .set_form([
                ("email", "PAT@example.com"),
                ("password", "not the password"),
            ])
            .to_request(),
    )
    .await;
    let unknown = call_service(
        &app,
        TestRequest::post()
            .uri("/auth/login")
            .set_form([("email", "nobody@example.com"), ("password", PASSWORD)])
            .to_request(),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(unknown.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(body_string(wrong).await, body_string(unknown).await);
}

/// P2 through the route: registering an address that differs from an
/// account's only by case or whitespace gets the duplicate-email answer,
/// and creates nothing.
#[actix_web::test]
async fn register_refuses_case_variant() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "pat@example.com").await;

    let token = invitation(&state).await;
    let resp = call_service(
        &app,
        register_request(Some(&token), " PAT@Example.COM ").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = body_string(resp).await;
    assert!(
        body.contains("An account already uses that email"),
        "{body}"
    );
    assert!(
        UserRepo::find_by_email(&state.pool, "PAT@Example.COM")
            .await
            .expect("lookup")
            .is_some_and(|(u, _)| u.email == "pat@example.com"),
        "the existing account, unchanged"
    );

    // A new address is stored trimmed, with its case as typed.
    let token = invitation(&state).await;
    let resp = call_service(
        &app,
        register_request(Some(&token), "  New.Person@Example.com ").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let (user, _) = UserRepo::find_by_email(&state.pool, "new.person@example.com")
        .await
        .expect("lookup")
        .expect("account created");
    assert_eq!(user.email, "New.Person@Example.com");
}

/// P3: the login limiter's account key and the sharing lookup agree with
/// sign-in: failures spelled in different cases (from different clients)
/// count against one account, and sharing to another spelling grants that
/// account.
#[actix_web::test]
async fn limiter_and_sharing_agree() {
    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "pat@example.com").await;
    let spellings = ["pat@example.com", "PAT@example.com", " Pat@Example.COM "];
    for i in 0..LOGIN_PER_ACCOUNT.failures {
        let typed = spellings[i as usize % spellings.len()];
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri("/auth/login")
                .peer_addr(addr(&format!("198.51.100.{}", i + 1)))
                .set_form([("email", typed), ("password", "not the password")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{typed:?}");
    }
    let resp = call_service(
        &app,
        login_request("PAT@EXAMPLE.COM")
            .peer_addr(addr("203.0.113.9"))
            .to_request(),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "one account, whatever the case"
    );

    // Sharing to another spelling grants the same account.
    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let pat = make_user(&state, "pat@example.com").await;
    let list = make_list(&state, owner, "Viikko").await;
    let session =
        session_cookie(&call_service(&app, login_request("OWNER@example.com").to_request()).await);
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/shares"))
            .cookie(session)
            .set_form([("email", " PAT@Example.com "), ("role", "view")])
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        ShoppingListRepo::access(&state.pool, pat, list)
            .await
            .expect("access"),
        ListAccess::View
    );
}
