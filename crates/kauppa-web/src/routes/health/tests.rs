//! MB-0046 P4 (RR-0029 sub-answer 4): `/healthz` reports whether the app can
//! reach its database, and nothing else.
#![allow(clippy::expect_used)]

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service, read_body};

use crate::routes::test_support::{test_app, test_state};

#[actix_web::test]
async fn reports_database_state() {
    let state = test_state().await;
    let app = test_app!(state);

    let res = call_service(&app, TestRequest::get().uri("/healthz").to_request()).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get(header::CONTENT_TYPE).map(|v| v.as_bytes()),
        Some(&b"text/plain"[..])
    );
    assert_eq!(read_body(res).await.as_ref(), b"ok");

    // The database is gone: 503 with a fixed body, no detail.
    state.pool.close().await;
    let res = call_service(&app, TestRequest::get().uri("/healthz").to_request()).await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        res.headers().get(header::CONTENT_TYPE).map(|v| v.as_bytes()),
        Some(&b"text/plain"[..])
    );
    assert_eq!(read_body(res).await.as_ref(), b"unavailable");
}
