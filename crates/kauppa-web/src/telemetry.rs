use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::Version;
use actix_web::{Error, HttpMessage, web};
use tracing::Span;
use tracing::field::Empty;
use tracing_actix_web::{DefaultRootSpanBuilder, RequestId, RootSpanBuilder};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::state::AppState;

pub fn init(default_level: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_env("KAUPPA_LOG")
        .or_else(|_| EnvFilter::try_new(default_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // JSON in release, pretty in debug — keeps dev readable, prod structured.
    #[cfg(debug_assertions)]
    let layer = fmt::layer().with_target(true).with_level(true).compact();

    #[cfg(not(debug_assertions))]
    let layer = fmt::layer().json().flatten_event(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("failed to install tracing subscriber: {e}"))?;

    Ok(())
}

/// The root span `TracingLogger` opens for every request: the fields of
/// tracing-actix-web's default span, except that `http.target` is the
/// request path only, never the query string (MB-0023, RR-0012
/// constraint 3). An invitation link's token is in the query, and both
/// formatters print the span's fields with every event in the request.
///
/// `http.client_ip` is the auth limiter's client key, truncated to IPv4 /24
/// or IPv6 /48 (MB-0026, RR-0013 sub-answer 6), never
/// `realip_remote_addr()`'s client-chosen address; empty when the request
/// has no peer address.
#[derive(Debug)]
pub struct RequestSpan;

impl RootSpanBuilder for RequestSpan {
    fn on_request_start(request: &ServiceRequest) -> Span {
        let user_agent = request
            .headers()
            .get("User-Agent")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let route = request
            .match_pattern()
            .unwrap_or_else(|| "default".to_owned());
        let method = request.method().as_str();
        let request_id = request
            .extensions()
            .get::<RequestId>()
            .map(ToString::to_string)
            .unwrap_or_default();
        let client_ip = request
            .app_data::<web::Data<AppState>>()
            .map(|state| {
                state
                    .limiter
                    .client_key(request.peer_addr(), request.headers())
                    .truncated()
                    .to_string()
            })
            .unwrap_or_default();
        let connection_info = request.connection_info();
        tracing::info_span!(
            "HTTP request",
            http.method = %method,
            http.route = %route,
            http.flavor = %flavor(request.version()),
            http.scheme = %connection_info.scheme(),
            http.host = %connection_info.host(),
            http.client_ip = %client_ip,
            http.user_agent = %user_agent,
            http.target = %request.path(),
            http.status_code = Empty,
            otel.name = %format!("{method} {route}"),
            otel.kind = "server",
            otel.status_code = Empty,
            trace_id = Empty,
            request_id = %request_id,
            exception.message = Empty,
            exception.details = Empty,
        )
    }

    fn on_request_end<B: MessageBody>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
        DefaultRootSpanBuilder::on_request_end(span, outcome);
    }
}

fn flavor(version: Version) -> String {
    match version {
        Version::HTTP_09 => "0.9".to_owned(),
        Version::HTTP_10 => "1.0".to_owned(),
        Version::HTTP_11 => "1.1".to_owned(),
        Version::HTTP_2 => "2.0".to_owned(),
        Version::HTTP_3 => "3.0".to_owned(),
        other => format!("{other:?}"),
    }
}
