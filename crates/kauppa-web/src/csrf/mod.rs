//! App-wide cross-site request protection (MB-0018, RR-0010).
//!
//! One middleware, [`guard`], sits in front of every route
//! (`routes::configure` wraps them all in one scope). Every non-safe request
//! (anything but GET, HEAD and OPTIONS) must prove it came from the app's own
//! pages, checked in this order:
//!
//! 1. `Sec-Fetch-Site: cross-site` or `same-site` → 403. Reject only: any
//!    other value, or none, falls through.
//! 2. `Origin` present → it must equal the app's origin. `Origin: null` is a
//!    mismatch.
//! 3. No `Origin` → the `Referer`'s scheme, host and port must equal the
//!    app's origin.
//! 4. Neither → 403.
//!
//! The app's origin is `KAUPPA_PUBLIC_ORIGIN` when set, otherwise the
//! request's own `Host` with the scheme from `KAUPPA_COOKIE_SECURE`
//! ([`OriginPolicy`]). `X-Forwarded-*` is never consulted.
//!
//! A rejection runs no handler, so it has no side effects. It answers with
//! the same plain 403 as any other "forbidden", and logs the method, path
//! and reason at WARN — never cookies, bodies or query strings.

mod origin;

#[cfg(test)]
mod tests;

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header::{self, HeaderMap};
use actix_web::http::{Method, Uri};
use actix_web::middleware::Next;
use actix_web::{Error, HttpResponse, web};

pub use origin::{AppOrigin, PublicOrigin, Scheme};

use crate::state::AppState;

/// How the app knows its own origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginPolicy {
    /// `KAUPPA_PUBLIC_ORIGIN`: compared exactly, whatever `Host` says.
    Fixed(AppOrigin),
    /// Unset: the request's own `Host`, with this scheme.
    SameHost(Scheme),
}

impl OriginPolicy {
    /// The policy for the settings: the public origin if one is configured,
    /// otherwise same-host with `https` when cookies are `Secure`.
    pub fn new(public_origin: Option<PublicOrigin>, cookie_secure: bool) -> Self {
        match public_origin {
            Some(PublicOrigin(origin)) => Self::Fixed(origin),
            None if cookie_secure => Self::SameHost(Scheme::Https),
            None => Self::SameHost(Scheme::Http),
        }
    }

    /// The app's origin for a request with these headers, if it has one: a
    /// same-host request without a usable `Host` has none. Also builds the
    /// admin portal's invitation links (MB-0023).
    pub fn app_origin(&self, headers: &HeaderMap, uri: &Uri) -> Option<AppOrigin> {
        match self {
            Self::Fixed(origin) => Some(origin.clone()),
            Self::SameHost(scheme) => {
                let host = match headers.get(header::HOST) {
                    Some(value) => value.to_str().ok()?,
                    None => uri.authority()?.as_str(),
                };
                AppOrigin::from_authority(*scheme, host)
            }
        }
    }
}

/// Why a request was rejected. Logged, never sent to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    /// `Sec-Fetch-Site` said `cross-site` or `same-site`.
    FetchMetadata,
    /// `Origin: null`.
    NullOrigin,
    /// `Origin` isn't the app's origin (or isn't an origin at all).
    OriginMismatch { got: Option<AppOrigin> },
    /// No `Origin`, and the `Referer` isn't from the app's origin.
    RefererMismatch { got: Option<AppOrigin> },
    /// Neither `Origin` nor `Referer`.
    MissingProof,
    /// Same-host policy, but the request has no usable `Host`.
    UnknownAppOrigin,
}

impl Rejection {
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::FetchMetadata => "cross-site fetch metadata",
            Self::NullOrigin => "null origin",
            Self::OriginMismatch { .. } => "origin mismatch",
            Self::RefererMismatch { .. } => "referer mismatch",
            Self::MissingProof => "missing origin and referer",
            Self::UnknownAppOrigin => "unknown app origin",
        }
    }

    /// The request's (normalized) origin, when it sent a parsable one.
    const fn got(&self) -> Option<&AppOrigin> {
        match self {
            Self::OriginMismatch { got } | Self::RefererMismatch { got } => got.as_ref(),
            _ => None,
        }
    }
}

/// GET, HEAD and OPTIONS. They stay side-effect free (RR-0010 constraint 2),
/// which is why they skip the check; every other method is checked.
pub const fn is_safe(method: &Method) -> bool {
    matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// The cross-site check for a non-safe request (steps 1–4 above). On
/// rejection, also returns the app's origin when it is known, for the log.
pub fn check(
    policy: &OriginPolicy,
    headers: &HeaderMap,
    uri: &Uri,
) -> Result<(), (Rejection, Option<AppOrigin>)> {
    // 1. Fetch Metadata, reject only.
    let fetch_site = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok());
    if fetch_site.is_some_and(|site| {
        site.eq_ignore_ascii_case("cross-site") || site.eq_ignore_ascii_case("same-site")
    }) {
        return Err((Rejection::FetchMetadata, None));
    }

    let Some(expected) = policy.app_origin(headers, uri) else {
        return Err((Rejection::UnknownAppOrigin, None));
    };
    let fail = |rejection| Err((rejection, Some(expected.clone())));

    // 2. Origin. Exactly one header; a repeated one is a mismatch.
    let mut origins = headers.get_all(header::ORIGIN);
    if let Some(value) = origins.next() {
        if origins.next().is_some() {
            return fail(Rejection::OriginMismatch { got: None });
        }
        let value = value.to_str().unwrap_or_default();
        if value == "null" {
            return fail(Rejection::NullOrigin);
        }
        let got = AppOrigin::parse(value);
        return if got.as_ref() == Some(&expected) {
            Ok(())
        } else {
            fail(Rejection::OriginMismatch { got })
        };
    }

    // 3. Referer fallback.
    let mut referers = headers.get_all(header::REFERER);
    if let Some(value) = referers.next() {
        let got = if referers.next().is_some() {
            None
        } else {
            value.to_str().ok().and_then(AppOrigin::from_referer)
        };
        return if got.as_ref() == Some(&expected) {
            Ok(())
        } else {
            fail(Rejection::RefererMismatch { got })
        };
    }

    // 4. No proof.
    fail(Rejection::MissingProof)
}

/// The middleware. Safe methods pass untouched; every other request passes
/// [`check`] against the [`OriginPolicy`] in [`AppState`] or gets 403. With
/// no `AppState` it fails closed.
pub async fn guard(
    req: ServiceRequest,
    next: Next<impl MessageBody + 'static>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    if is_safe(req.method()) {
        return next
            .call(req)
            .await
            .map(ServiceResponse::map_into_left_body);
    }
    let verdict = req.app_data::<web::Data<AppState>>().map_or_else(
        || Err((Rejection::UnknownAppOrigin, None)),
        |state| check(&state.origin, req.headers(), req.uri()),
    );
    match verdict {
        Ok(()) => next
            .call(req)
            .await
            .map(ServiceResponse::map_into_left_body),
        Err((rejection, expected)) => {
            tracing::warn!(
                method = %req.method(),
                path = req.path(),
                reason = rejection.reason(),
                origin = rejection.got().map(AppOrigin::as_str),
                expected = expected.as_ref().map(AppOrigin::as_str),
                "cross-site request rejected"
            );
            // The same plain answer as any other 403 (`WebError`), so the
            // client can't tell which check failed.
            let resp = HttpResponse::Forbidden().body("forbidden".to_owned());
            Ok(req.into_response(resp).map_into_right_body())
        }
    }
}
