//! The app's security headers (MB-0031, RR-0019): an enforcing
//! Content-Security-Policy and three companion headers, sent on every
//! response by one [`DefaultHeaders`] that `main.rs` wraps outermost (and
//! `wired_app!` mirrors), so the session and identity middleware, the
//! cross-site check's 403, `/assets`, extractor errors and the catch-all 404
//! all carry them.
//!
//! [`DefaultHeaders`] only inserts a header the response doesn't already
//! have, so a handler may send a stricter one of its own (the register
//! page's `Referrer-Policy: no-referrer`, RR-0012). A handler must never
//! weaken or remove these (RR-0019 constraint 5), and loosening the policy
//! needs an RR (constraint 2).

use actix_web::http::header;
use actix_web::middleware::DefaultHeaders;

/// The policy, identical for every response (RR-0019 sub-answer 1). No
/// `'unsafe-inline'`, `'unsafe-eval'`, nonce, hash or `data:`: templates
/// load only same-origin scripts and styles, and htmx runs with `allowEval`
/// and `includeIndicatorStyles` off (`layout.hbs`'s `htmx-config`).
pub const POLICY: &str = "default-src 'self'; script-src 'self'; style-src 'self'; \
                          img-src 'self'; object-src 'none'; base-uri 'none'; \
                          form-action 'self'; frame-ancestors 'none'";

/// HSTS, sent only when cookies are `Secure` (the deployment serves HTTPS):
/// one year, no `includeSubDomains`, no `preload` (RR-0019 constraint 6).
pub const HSTS: &str = "max-age=31536000";

/// The security headers middleware. `cookie_secure` is
/// `KAUPPA_COOKIE_SECURE`, the deployment's HTTPS signal, which turns HSTS
/// on.
pub fn headers(cookie_secure: bool) -> DefaultHeaders {
    let headers = DefaultHeaders::new()
        .add((header::CONTENT_SECURITY_POLICY, POLICY))
        .add((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .add((header::REFERRER_POLICY, "same-origin"));
    if cookie_secure {
        headers.add((header::STRICT_TRANSPORT_SECURITY, HSTS))
    } else {
        headers
    }
}

#[cfg(test)]
mod tests;
