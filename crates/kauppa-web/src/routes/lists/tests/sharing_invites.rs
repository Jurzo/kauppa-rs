//! MB-0008 (P5, P7): enumeration-safe invites and audit logging.

use actix_web::http::{StatusCode, header};

use kauppa_core::repo::ShoppingListRepo;

use crate::routes::test_support::LogCapture;

/// MB-0008 P5: sharing to an email with an account, one without, and the
/// owner's own answer identically — status, `Location` and body, as a plain
/// post and over htmx — while only the first grants access.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn share_is_enumeration_safe() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        body_string, login_request, make_user, session_cookie, test_app, test_state,
    };

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let known = make_user(&state, "known@example.com").await;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("owner@example.com").to_request()).await);

    for htmx in [false, true] {
        let mut answers = Vec::new();
        for email in [
            "known@example.com",
            "nobody@example.com",
            "KNOWN@example.com",
            "owner@example.com",
            "not-an-email",
        ] {
            let mut req = TestRequest::post()
                .uri(&format!("/lists/{list}/shares"))
                .cookie(session.clone())
                .set_form([("email", email), ("role", "edit")]);
            if htmx {
                req = req.insert_header(("HX-Request", "true"));
            }
            let resp = call_service(&app, req.to_request()).await;
            let status = resp.status();
            let location = resp
                .headers()
                .get(header::LOCATION)
                .map(|v| v.to_str().expect("ascii").to_owned());
            let content_type = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .map(|v| v.to_str().expect("ascii").to_owned());
            let body = body_string(resp).await;
            assert!(!body.contains(email), "htmx={htmx}: the email isn't echoed");
            answers.push((email, (status, location, content_type, body)));
        }
        let (_, first) = &answers[0];
        let expected = if htmx {
            StatusCode::OK
        } else {
            StatusCode::SEE_OTHER
        };
        assert_eq!(first.0, expected, "htmx={htmx}");
        if htmx {
            assert!(first.3.contains("If an account uses that email"));
        }
        for (email, answer) in &answers[1..] {
            assert_eq!(answer, first, "htmx={htmx}: {email} answers like a match");
        }
    }

    // Only the real account was granted (once; sharing again updates it).
    let shares = ShoppingListRepo::shares(&state.pool, owner, list)
        .await
        .expect("shares");
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].grantee_id, known);

    // A rejected input is a different answer, but it says nothing about
    // accounts either: it is the same for both emails.
    let mut rejections = Vec::new();
    for email in ["known@example.com", "nobody@example.com"] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&format!("/lists/{list}/shares"))
                .cookie(session.clone())
                .insert_header(("HX-Request", "true"))
                .set_form([("email", email), ("role", "admin")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = body_string(resp).await.replace(email, "EMAIL");
        assert!(body.contains("choose view or edit"));
        rejections.push(body);
    }
    assert_eq!(rejections[0], rejections[1]);
}

/// MB-0008 P7: granting and revoking log, at `info`, the grantor and the list
/// id (and the grant's ids, never an email); nothing at `info` or above
/// carries the grantee's email, matched or not.
#[actix_web::test]
#[allow(clippy::expect_used)]
async fn share_audit_logging() {
    use actix_web::test::{TestRequest, call_service};

    use crate::routes::test_support::{
        login_request, make_user, session_cookie, test_app, test_state,
    };

    // Tests run in parallel. With a single registered dispatcher, tracing
    // computes a new callsite's interest from the *registering* thread's
    // subscriber, so another test's thread (which has none) could switch the
    // share callsites off for this one. A second, idle dispatcher makes it
    // ask every registered subscriber instead.
    let _idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let capture = LogCapture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let state = test_state().await;
    let app = test_app!(state);
    let owner = make_user(&state, "owner@example.com").await;
    let grantee = make_user(&state, "secret.grantee@example.com").await;
    let list = ShoppingListRepo::create(&state.pool, owner, "Kauppa")
        .await
        .expect("list")
        .id;
    let session =
        session_cookie(&call_service(&app, login_request("owner@example.com").to_request()).await);

    // Callsites another test registered before this subscriber existed:
    // recompute their interest now that it does.
    tracing::callsite::rebuild_interest_cache();
    for email in ["secret.grantee@example.com", "secret.nobody@example.com"] {
        let resp = call_service(
            &app,
            TestRequest::post()
                .uri(&format!("/lists/{list}/shares"))
                .cookie(session.clone())
                .set_form([("email", email), ("role", "edit")])
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    }
    let grant = ShoppingListRepo::shares(&state.pool, owner, list)
        .await
        .expect("shares")
        .pop()
        .expect("granted");
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri(&format!("/lists/{list}/shares/{}/revoke", grant.id))
            .cookie(session.clone())
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    let log = capture.text();
    let line = |message: &str| {
        log.lines()
            .find(|l| l.contains(message))
            .unwrap_or_default()
            .to_owned()
    };
    for message in [
        "list share granted",
        "list share requested; no single other account matched",
        "list share revoked",
    ] {
        let line = line(message);
        assert!(line.contains(" INFO "), "{message} at info: {log}");
        assert!(
            line.contains(&format!("grantor_id={owner}")),
            "{message}: {line}"
        );
        assert!(
            line.contains(&format!("list_id={list}")),
            "{message}: {line}"
        );
    }
    for message in ["list share granted", "list share revoked"] {
        let line = line(message);
        assert!(
            line.contains(&format!("grantee_id={grantee}")),
            "{message}: {line}"
        );
        assert!(
            line.contains(&format!("share_id={}", grant.id)),
            "{message}: {line}"
        );
    }
    assert!(
        !log.contains("secret."),
        "no email (or its local part) at info or above: {log}"
    );
}
