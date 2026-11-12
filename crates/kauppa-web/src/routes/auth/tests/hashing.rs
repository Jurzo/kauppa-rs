//! MB-0046 P1 (RR-0029 sub-answer 1, constraints 1 and 2): password hashing
//! runs on actix's blocking pool, never on an async worker, with the same
//! results and the same single verification per sign-in.

use std::thread;

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};

use crate::routes::auth::hashing;
use crate::routes::test_support::{
    PASSWORD, login_request, make_user, source_files, test_app, test_state,
};

/// The helper runs its work on another thread than the request's, and the
/// hash and verify wrappers answer as `password` does; sign-in still works
/// for a right password and refuses a wrong one or an unknown email alike.
#[actix_web::test]
async fn runs_off_the_async_workers() {
    let here = thread::current().id();
    let there = hashing::on_blocking_pool(|| Ok(thread::current().id()))
        .await
        .expect("ran on the pool");
    assert_ne!(here, there, "hashing ran on the async worker");

    let hash = hashing::hash(PASSWORD.to_owned()).await.expect("hash");
    assert!(
        hashing::verify(PASSWORD.to_owned(), hash.clone())
            .await
            .expect("verify")
    );
    assert!(
        !hashing::verify("not the password".to_owned(), hash)
            .await
            .expect("verify")
    );
    // The unknown-email path: one verification against the dummy hash,
    // whose answer is never a match that matters.
    hashing::verify_dummy("anything at all".to_owned())
        .await
        .expect("dummy verify");

    let state = test_state().await;
    let app = test_app!(state);
    make_user(&state, "hash@example.com").await;
    let ok = call_service(&app, login_request("hash@example.com").to_request()).await;
    assert_eq!(ok.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        ok.headers()
            .get(header::LOCATION)
            .map(actix_web::http::header::HeaderValue::as_bytes),
        Some(&b"/lists"[..])
    );
    for (email, password) in [
        ("hash@example.com", "a wrong password here"),
        ("nobody@example.com", PASSWORD),
    ] {
        let res = call_service(
            &app,
            TestRequest::post()
                .uri("/auth/login")
                .set_form([("email", email), ("password", password)])
                .to_request(),
        )
        .await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "{email}");
    }
}

/// RR-0029 constraint 1: outside the helper, no non-test source in the web
/// crate calls `password::hash` or `password::verify` directly.
#[test]
fn handlers_never_hash_directly() {
    for (file, text) in source_files() {
        if file == "routes/auth/hashing.rs" {
            continue;
        }
        for (i, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for call in ["password::hash(", "password::verify("] {
                assert!(
                    !code.contains(call),
                    "{file}:{}: `{call}` outside routes/auth/hashing.rs (RR-0029)",
                    i + 1
                );
            }
        }
    }
}
