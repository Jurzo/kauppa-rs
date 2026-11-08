//! RR-0012 sub-answer 5 and constraint 3: request log spans record the
//! path only, so an invitation token in the query never reaches a log line.

use actix_web::http::StatusCode;
use actix_web::test::{TestRequest, call_service, init_service};
use tracing_actix_web::TracingLogger;
use tracing_subscriber::fmt::format::FmtSpan;

use super::{invitation, register_request};
use crate::routes::test_support::{
    LogCapture, add_default_proof, login_request, session_cookie, test_state, wired_app,
};
use crate::telemetry::RequestSpan;

#[actix_web::test]
async fn token_never_logged() {
    // As in `error::tests`: a second, idle dispatcher keeps parallel tests
    // from switching callsites off; the rebuild recomputes interest.
    let _idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let capture = LogCapture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_max_level(tracing::Level::TRACE)
        // Print every span with its fields when it opens and closes, on top
        // of the fields every event inside it carries.
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let state = test_state().await;
    // Wired as `main.rs` does, with the request logger.
    let app = init_service(
        wired_app!(state)
            .wrap(TracingLogger::<RequestSpan>::new())
            .wrap_fn(|mut req, srv| {
                add_default_proof(&mut req);
                actix_web::dev::Service::call(srv, req)
            }),
    )
    .await;
    let token = invitation(&state).await;

    // The invitation link, a use, and a reuse (the 400 answer).
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/auth/register?invite={token}"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    for expected in [StatusCode::SEE_OTHER, StatusCode::BAD_REQUEST] {
        let resp = call_service(
            &app,
            register_request(Some(&token), "pat@example.com").to_request(),
        )
        .await;
        assert_eq!(resp.status(), expected);
    }
    // A token in the query of a request that logs a WARN (a list that
    // isn't there, signed in as the new account).
    let resp = call_service(&app, login_request("pat@example.com").to_request()).await;
    let session = session_cookie(&resp);
    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/lists/{}?invite={token}", uuid::Uuid::nil()))
            .cookie(session)
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let log = capture.text();
    // The spans were logged, with the path.
    assert!(
        log.contains("http.target=/auth/register "),
        "path recorded: {log}"
    );
    let warn = log
        .lines()
        .find(|l| l.contains(" WARN ") && l.contains("request error"))
        .unwrap_or_else(|| panic!("a WARN line: {log}"));
    assert!(
        warn.contains(&format!("http.target=/lists/{}", uuid::Uuid::nil())),
        "{warn}"
    );
    assert!(!log.contains("invite="), "no query in the log: {log}");
    assert!(!log.contains(&token), "token in the log: {log}");
    assert!(!log.contains(&kauppa_core::token::hash(&token)), "{log}");
}
