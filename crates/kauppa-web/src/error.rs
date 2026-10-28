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
            Self::Core(CoreError::Conflict(_)) => StatusCode::CONFLICT,
            Self::Core(CoreError::Invalid(_)) => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        // Log the full error server-side; return a sanitized message to the client.
        tracing::error!(error = ?self, "request error");
        let status = self.status_code();
        let safe_message = match self {
            Self::Core(CoreError::Invalid(m) | CoreError::Conflict(m)) => m.clone(),
            Self::Core(CoreError::NotFound) => "not found".to_owned(),
            Self::Core(CoreError::Unauthorized) => "unauthorized".to_owned(),
            _ => "internal server error".to_owned(),
        };
        HttpResponse::build(status).body(safe_message)
    }
}
