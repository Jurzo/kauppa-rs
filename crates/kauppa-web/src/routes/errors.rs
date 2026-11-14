//! Error pages (MB-0056, RR-0038).
//!
//! [`pages`] wraps the RR-0010 scope, outside `csrf::guard`
//! (`routes::configure`), so it sees every answer a route, an extractor,
//! the router or the cross-site check gives. It replaces an error answer
//! that isn't already a page with the error page for its status, in the
//! request's language and theme, keeping the status:
//!
//! - **Which answers.** A 4xx or 5xx that carries an error (a `WebError`,
//!   an extractor's or the identity middleware's) or has no `Content-Type`
//!   (the router's 404 and 405, the guard's 403). A page a handler renders
//!   on purpose sets `Content-Type: text/html` and passes through, as does
//!   `/healthz`'s `text/plain` 503.
//! - **What it says.** Only what the status says ([`ErrorPage`]): never why
//!   the request was refused or what it carried (RR-0038 constraints 1 and
//!   2). The cross-site 403 and an authorization 403 are the same page.
//! - **Signed out.** The 401 page holds the sign-in form; for a page (a GET
//!   that isn't an htmx fragment request) it returns there after signing in.
//! - **htmx.** The page tells htmx to replace `<body>` (`HX-Retarget`,
//!   `HX-Reswap`), so it never lands in a fragment's target.
//! - **Fallback.** If the page can't be made, the original answer goes out
//!   unchanged and one ERROR line names the template (P5).
//!
//! It must stay inside the identity and session middleware (RR-0038
//! constraint 3): actix-identity's `Identity` extractor panics without them.
//! The middleware never holds a copy of the request across the inner call:
//! actix-web's router needs it unshared (CRP-0038, trial point 6). It reads
//! the request back from the response instead.

use actix_identity::Identity;
use actix_web::body::{BoxBody, MessageBody};
use actix_web::dev::{Payload, ServiceRequest, ServiceResponse};
use actix_web::http::header::{self, HeaderName, HeaderValue};
use actix_web::http::{Method, StatusCode};
use actix_web::middleware::Next;
use actix_web::{Error, FromRequest, HttpRequest, HttpResponse};
use serde::Serialize;

use crate::i18n::messages::{self, ErrorPage};
use crate::i18n::{Ui, VARY};
use crate::routes::request::{current_user, is_hx, wants_fragment};

#[cfg(test)]
mod tests;

/// The error page's template.
const PAGE: &str = "errors/page";

/// What `errors/page` shows.
#[derive(Debug, Serialize)]
struct ErrorView {
    title: String,
    text: String,
    signed_in: bool,
    /// Show the sign-in form (a 401).
    sign_in: bool,
    /// The sign-in form's `return`: the page to go back to.
    return_to: Option<String>,
}

/// The middleware around the RR-0010 scope.
pub async fn pages(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let res = next.call(req).await?.map_into_boxed_body();
    if !is_plain_error(&res) {
        return Ok(res);
    }
    let status = res.status();
    let Some(body) = render(res.request(), status).await else {
        tracing::error!(
            template = PAGE,
            status = status.as_u16(),
            "error page not rendered; sending the plain answer"
        );
        return Ok(res);
    };
    let page = page_response(&res, status, body);
    Ok(res.into_response(page))
}

/// An error status whose answer isn't a page a handler made on purpose.
fn is_plain_error<B>(res: &ServiceResponse<B>) -> bool {
    let status = res.status();
    (status.is_client_error() || status.is_server_error())
        && (res.response().error().is_some() || !res.headers().contains_key(header::CONTENT_TYPE))
}

/// The page for `status`, or `None` if it can't be made.
async fn render(req: &HttpRequest, status: StatusCode) -> Option<String> {
    let ui = Ui::from_request(req, &mut Payload::None).await.ok()?;
    let signed_in = Identity::from_request(req, &mut Payload::None)
        .into_inner()
        .is_ok_and(|id| current_user(&id).is_ok());
    let page = ErrorPage::for_status(status);
    let (title, text) = messages::error_page(page);
    let sign_in = page == ErrorPage::SignedOut;
    let return_to = (sign_in && req.method() == Method::GET && !wants_fragment(req)).then(|| {
        req.uri()
            .path_and_query()
            .map_or_else(|| "/".to_owned(), |p| p.as_str().to_owned())
    });
    let view = ErrorView {
        title: ui.say(title).ok()?,
        text: ui.say(text).ok()?,
        signed_in,
        sign_in,
        return_to,
    };
    ui.render(ui.state(), PAGE, &view).ok()
}

/// `body` as the answer, with `res`'s status and headers except its body's
/// own, as HTML; under htmx, told to replace `<body>`.
fn page_response<B>(res: &ServiceResponse<B>, status: StatusCode, body: String) -> HttpResponse {
    let mut page = HttpResponse::build(status)
        .content_type("text/html; charset=utf-8")
        .body(body);
    let headers = page.headers_mut();
    for (name, value) in res.headers() {
        if name != header::CONTENT_TYPE && name != header::CONTENT_LENGTH {
            headers.append(name.clone(), value.clone());
        }
    }
    let (vary, vary_value) = VARY;
    headers.insert(vary, HeaderValue::from_static(vary_value));
    if is_hx(res.request()) {
        headers.insert(
            HeaderName::from_static("hx-retarget"),
            HeaderValue::from_static("body"),
        );
        headers.insert(
            HeaderName::from_static("hx-reswap"),
            HeaderValue::from_static("innerHTML"),
        );
    }
    page
}
