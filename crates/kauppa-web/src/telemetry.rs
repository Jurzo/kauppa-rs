use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::Version;
use actix_web::{Error, HttpMessage};
use tracing::Span;
use tracing::field::Empty;
use tracing_actix_web::{DefaultRootSpanBuilder, RequestId, RootSpanBuilder};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

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
/// MB-0026 adds its resolved `http.client_ip` here (CRP-0013 Q6).
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
        let connection_info = request.connection_info();
        tracing::info_span!(
            "HTTP request",
            http.method = %method,
            http.route = %route,
            http.flavor = %flavor(request.version()),
            http.scheme = %connection_info.scheme(),
            http.host = %connection_info.host(),
            http.client_ip = %connection_info.realip_remote_addr().unwrap_or(""),
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
