//! MB-0018 P6: every rejection logs one WARN line with the method, path and
//! reason, and nothing secret: no cookie, no form body, no email.

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};

use super::{EDITOR, OWNER, World, world, write_routes};
use crate::routes::test_support::{
    LogCapture, TEST_HOST, TEST_ORIGIN, bare_test_app, login_request, session_cookie, test_state,
};

type Headers = &'static [(&'static str, &'static str)];

#[actix_web::test]
async fn rejections_are_logged() {
    // As in `error::tests`: a second, idle dispatcher keeps parallel tests
    // from switching this callsite off, and the rebuild recomputes interest
    // for callsites registered earlier.
    let _idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let capture = LogCapture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();

    let state = test_state().await;
    let app = bare_test_app!(state);
    let w: World = world(&state).await;
    let login = login_request(OWNER)
        .insert_header((header::HOST, TEST_HOST))
        .insert_header((header::ORIGIN, TEST_ORIGIN));
    let session = session_cookie(&call_service(&app, login.to_request()).await);

    // (headers, logged reason, logged origin)
    let cases: &[(Headers, &str, Option<&str>)] = &[
        (
            &[("Sec-Fetch-Site", "cross-site"), ("Origin", TEST_ORIGIN)],
            "cross-site fetch metadata",
            None,
        ),
        (
            &[("Origin", "http://evil.example")],
            "origin mismatch",
            Some("http://evil.example"),
        ),
        (&[("Origin", "null")], "null origin", None),
        (
            &[("Referer", "http://evil.example/secret.page?token=abc")],
            "referer mismatch",
            Some("http://evil.example"),
        ),
        (&[], "missing origin and referer", None),
    ];

    let share = write_routes()
        .into_iter()
        .find(|r| r.path == "/lists/{id}/shares")
        .expect("share route");
    let uri = w.uri(&share);
    for &(headers, reason, origin) in cases {
        let mut req = TestRequest::post()
            .uri(&format!("{uri}?email={EDITOR}"))
            .insert_header((header::HOST, TEST_HOST))
            .cookie(session.clone())
            .set_form(w.form(&share));
        for &(name, value) in headers {
            req = req.insert_header((name, value));
        }
        let before = capture.text();
        let resp = call_service(&app, req.to_request()).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{reason}");

        let log = capture.text();
        let new = &log[before.len()..];
        let lines: Vec<_> = new
            .lines()
            .filter(|l| l.contains("cross-site request rejected"))
            .collect();
        assert_eq!(lines.len(), 1, "{reason}: one line: {new}");
        let line = lines[0];
        assert!(line.contains(" WARN kauppa_web::csrf:"), "{line}");
        assert!(line.contains("method=POST"), "{line}");
        assert!(
            line.contains(&format!("path=\"{}\"", w.uri(&share))),
            "{line}"
        );
        assert!(line.contains(&format!("reason=\"{reason}\"")), "{line}");
        match origin {
            Some(o) => assert!(line.contains(&format!("origin=\"{o}\"")), "{line}"),
            None => assert!(!line.contains("origin=\""), "{line}"),
        }
        assert!(
            line.contains(&format!("expected=\"{TEST_ORIGIN}\"")) || reason.contains("fetch"),
            "{line}"
        );
        assert!(
            !new.contains(" ERROR "),
            "{reason}: nothing at ERROR: {new}"
        );
    }

    // Nothing secret anywhere in the log.
    let log = capture.text();
    for secret in [
        session.value(),
        "kauppa_sid",
        EDITOR,
        "editor",
        "Hijacked",
        "secret.page",
        "token=abc",
        crate::routes::test_support::PASSWORD,
    ] {
        assert!(!log.contains(secret), "{secret:?} in log: {log}");
    }
}
