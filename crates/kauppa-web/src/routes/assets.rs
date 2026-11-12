//! Static files under `/assets/`, compiled into the binary (MB-0024).
//! `build.rs` lists every file in the workspace's `assets/` with its content
//! type; this service answers `GET` and `HEAD` for exactly those paths and
//! nothing else, as the folder was served before MB-0024.
//!
//! Caching (MB-0046, RR-0029 sub-answer 3): files under `vendor/` carry
//! their version in the name and are never edited (RR-0015), so browsers
//! may keep them a year; every other file keeps its name across deploys, so
//! it is revalidated against its `ETag` (304 when unchanged).
//!
//! It is a service factory, not a handler attribute or builder route: it
//! registers one prefix of its own, beside `routes::configure`'s cross-site
//! scope rather than inside it, and it accepts no write method (MB-0018 P4).

use std::future::{Ready, ready};

use actix_web::dev::{
    AppService, HttpServiceFactory, ResourceDef, ServiceRequest, ServiceResponse, fn_service,
};
use actix_web::http::{Method, header};
use actix_web::{Error, HttpResponse};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// Where the files are served.
pub const MOUNT: &str = "/assets";

/// The embedded files at [`MOUNT`]: `App::new().service(Assets)`.
#[derive(Debug, Clone, Copy)]
pub struct Assets;

impl HttpServiceFactory for Assets {
    fn register(self, config: &mut AppService) {
        let rdef = if config.is_root() {
            ResourceDef::root_prefix(MOUNT)
        } else {
            ResourceDef::prefix(MOUNT)
        };
        config.register_service(rdef, None, fn_service(serve), None);
    }
}

/// The cache policy for a versioned, never-edited file (`vendor/`).
pub const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// The cache policy for every other file: keep it, but ask first.
pub const REVALIDATE: &str = "no-cache";

fn serve(req: ServiceRequest) -> Ready<Result<ServiceResponse, Error>> {
    let if_none_match = req
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok());
    let res = respond(req.method(), req.match_info().unprocessed(), if_none_match);
    ready(Ok(req.into_response(res)))
}

/// Whether an `If-None-Match` value matches `etag`: `*`, or a
/// comma-separated list holding it (a weak `W/` form compares equal, as
/// RFC 9110's weak comparison for `If-None-Match` says).
fn matches(if_none_match: &str, etag: &str) -> bool {
    if_none_match
        .split(',')
        .map(str::trim)
        .any(|given| given == "*" || given.strip_prefix("W/").unwrap_or(given) == etag)
}

/// The response to `method` on `path`, the request path after [`MOUNT`]
/// (`/app.css`). Only an exact, embedded name matches: there is no
/// decoding, normalizing or directory lookup, so `..`, `%2e%2e`, `//` or a
/// trailing `/` can't reach anything but a 404.
fn respond(method: &Method, path: &str, if_none_match: Option<&str>) -> HttpResponse {
    if method != Method::GET && method != Method::HEAD {
        return HttpResponse::MethodNotAllowed()
            .insert_header((header::ALLOW, "GET, HEAD"))
            .finish();
    }
    let found = path
        .strip_prefix('/')
        .and_then(|name| ASSETS.iter().find(|(n, ..)| *n == name));
    let Some((name, content_type, etag, bytes)) = found else {
        return HttpResponse::NotFound().finish();
    };
    let cache = if name.starts_with("vendor/") {
        IMMUTABLE
    } else {
        REVALIDATE
    };
    if if_none_match.is_some_and(|given| matches(given, etag)) {
        return HttpResponse::NotModified()
            .insert_header((header::ETAG, *etag))
            .insert_header((header::CACHE_CONTROL, cache))
            .finish();
    }
    // HEAD gets the same response; actix-http sends its headers only.
    HttpResponse::Ok()
        .insert_header((header::CONTENT_TYPE, *content_type))
        .insert_header((header::ETAG, *etag))
        .insert_header((header::CACHE_CONTROL, cache))
        .body(*bytes)
}

#[cfg(test)]
mod tests;
