//! HTTP error adapter for `kauppa_core::CoreError`.
//!
//! All route handlers return `WebResult<T>` so core errors map to HTTP without
//! leaking internals and without panicking. Add new variants here rather than
//! sprinkling `.map_err(ErrorInternalServerError)` through handlers.

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use kauppa_core::CoreError;

pub type WebResult<T> = Result<T, WebError>;

#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error(transparent)]
    Core(#[from] CoreError),

    #[error("template render error: {0}")]
    Template(#[from] handlebars::RenderError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

impl ResponseError for WebError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Core(CoreError::NotFound) => StatusCode::NOT_FOUND,
            Self::Core(CoreError::Unauthorized) => StatusCode::UNAUTHORIZED,
            Self::Core(CoreError::Forbidden) => StatusCode::FORBIDDEN,
            Self::Core(CoreError::Conflict(_)) => StatusCode::CONFLICT,
            Self::Core(CoreError::Invalid(_)) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        // Log the full error server-side; return a sanitized message to the client.
        // Client errors are answers the app gives on purpose, so they log at
        // WARN; ERROR is kept for server errors (MB-0020).
        let status = self.status_code();
        if status.is_client_error() {
            tracing::warn!(error = ?self, "request error");
        } else {
            tracing::error!(error = ?self, "request error");
        }
        let safe_message = match self {
            Self::Core(CoreError::Invalid(m) | CoreError::Conflict(m)) => m.clone(),
            Self::Core(CoreError::NotFound) => "not found".to_owned(),
            Self::Core(CoreError::Unauthorized) => "unauthorized".to_owned(),
            Self::Core(CoreError::Forbidden) => "forbidden".to_owned(),
            _ => "internal server error".to_owned(),
        };
        HttpResponse::build(status).body(safe_message)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use actix_web::http::StatusCode;
    use actix_web::{HttpResponse, ResponseError};
    use kauppa_core::CoreError;

    use super::WebError;
    use crate::routes::test_support::LogCapture;

    /// Every client error a handler can return.
    fn client_errors() -> Vec<WebError> {
        vec![
            WebError::Core(CoreError::Invalid("bad quantity".to_owned())),
            WebError::Core(CoreError::Unauthorized),
            WebError::Core(CoreError::Forbidden),
            WebError::Core(CoreError::NotFound),
            WebError::Core(CoreError::Conflict("email taken".to_owned())),
        ]
    }

    /// Every server error: template, IO, internal, and the core errors that
    /// map to 500.
    fn server_errors() -> Vec<WebError> {
        let render = handlebars::Handlebars::new()
            .render("missing", &())
            .expect_err("no such template");
        vec![
            WebError::Template(render),
            WebError::Io(std::io::Error::other("disk on fire")),
            WebError::Internal("secret internals".to_owned()),
            WebError::Core(CoreError::Database(sqlx::Error::RowNotFound)),
            WebError::Core(CoreError::PasswordHash("salt detail".to_owned())),
        ]
    }

    /// Builds `err`'s response with this thread's `tracing` output captured
    /// at INFO and above; returns the response and the log.
    fn respond_captured(err: &WebError) -> (HttpResponse, String) {
        // As in MB-0008's `share_audit_logging`: a second, idle dispatcher
        // keeps parallel tests from switching this callsite off, and the
        // rebuild recomputes interest for callsites registered earlier.
        let _idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        let capture = LogCapture::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(capture.clone())
            .with_max_level(tracing::Level::INFO)
            .with_ansi(false)
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        tracing::callsite::rebuild_interest_cache();
        let resp = err.error_response();
        (resp, capture.text())
    }

    /// The single `request error` line in `log`, checked to carry the same
    /// target, message and `error` field as before MB-0020.
    fn request_error_line(err: &WebError, log: &str) -> String {
        let lines: Vec<_> = log
            .lines()
            .filter(|l| l.contains("request error"))
            .collect();
        assert_eq!(lines.len(), 1, "{err:?} logged once: {log}");
        let line = lines[0];
        assert!(
            line.ends_with(&format!("kauppa_web::error: request error error={err:?}")),
            "{err:?}: same target, message and field: {line}"
        );
        line.to_owned()
    }

    /// MB-0020 P1: every client error logs once at WARN, nothing at ERROR.
    #[test]
    fn client_errors_log_at_warn() {
        for err in client_errors() {
            let (resp, log) = respond_captured(&err);
            assert!(resp.status().is_client_error(), "{err:?}");
            let line = request_error_line(&err, &log);
            assert!(line.contains(" WARN kauppa_web::error:"), "{err:?}: {line}");
            assert!(!log.contains("ERROR"), "{err:?}: nothing at ERROR: {log}");
        }
    }

    /// MB-0020 P2: server errors still log at ERROR.
    #[test]
    fn server_errors_log_at_error() {
        for err in server_errors() {
            let (resp, log) = respond_captured(&err);
            assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR, "{err:?}");
            let line = request_error_line(&err, &log);
            assert!(
                line.contains(" ERROR kauppa_web::error:"),
                "{err:?}: {line}"
            );
            assert!(!log.contains(" WARN "), "{err:?}: not at WARN: {log}");
        }
    }

    /// MB-0020 P3: status codes and sanitized bodies, per variant, as before;
    /// a server error's internals never reach the client.
    #[actix_web::test]
    async fn responses_unchanged() {
        let mut expected = vec![
            (StatusCode::BAD_REQUEST, "bad quantity"),
            (StatusCode::UNAUTHORIZED, "unauthorized"),
            (StatusCode::FORBIDDEN, "forbidden"),
            (StatusCode::NOT_FOUND, "not found"),
            (StatusCode::CONFLICT, "email taken"),
        ];
        expected.extend([(StatusCode::INTERNAL_SERVER_ERROR, "internal server error"); 5]);
        let errors: Vec<_> = client_errors().into_iter().chain(server_errors()).collect();
        assert_eq!(errors.len(), expected.len());
        for (err, (status, body)) in errors.iter().zip(expected) {
            assert_eq!(err.status_code(), status, "{err:?}");
            let resp = err.error_response();
            assert_eq!(resp.status(), status, "{err:?}");
            let bytes = actix_web::body::to_bytes(resp.into_body())
                .await
                .expect("body");
            assert_eq!(bytes, body.as_bytes(), "{err:?}: sanitized body");
        }
    }
}
