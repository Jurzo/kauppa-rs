//! Static files under `/assets/`, compiled into the binary (MB-0024).
//! `build.rs` lists every file in the workspace's `assets/` with its content
//! type; this service answers `GET` and `HEAD` for exactly those paths and
//! nothing else, the way `actix-files` did for the folder.
//!
//! It is a service factory, not a handler attribute or builder route: it
//! registers one prefix of its own, beside `routes::configure`'s cross-site
//! scope rather than inside it, the way `Files` was registered in `main.rs`,
//! and it accepts no write method (MB-0018 P4).

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

fn serve(req: ServiceRequest) -> Ready<Result<ServiceResponse, Error>> {
    let res = respond(req.method(), req.match_info().unprocessed());
    ready(Ok(req.into_response(res)))
}

/// The response to `method` on `path`, the request path after [`MOUNT`]
/// (`/app.css`). Only an exact, embedded name matches: there is no
/// decoding, normalizing or directory lookup, so `..`, `%2e%2e`, `//` or a
/// trailing `/` can't reach anything but a 404.
fn respond(method: &Method, path: &str) -> HttpResponse {
    if method != Method::GET && method != Method::HEAD {
        return HttpResponse::MethodNotAllowed()
            .insert_header((header::ALLOW, "GET, HEAD"))
            .finish();
    }
    let found = path
        .strip_prefix('/')
        .and_then(|name| ASSETS.iter().find(|(n, ..)| *n == name));
    match found {
        // HEAD gets the same response; actix-http sends its headers only.
        Some((_, content_type, bytes)) => HttpResponse::Ok()
            .insert_header((header::CONTENT_TYPE, *content_type))
            .body(*bytes),
        None => HttpResponse::NotFound().finish(),
    }
}

#[cfg(test)]
mod tests;
