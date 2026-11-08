//! Web origins (`scheme://host[:port]`), parsed strictly and normalized so
//! two spellings of the same origin compare equal (RR-0010).
//!
//! Normalization: scheme and host lower-cased, the scheme's default port
//! dropped. Only `http` and `https` are accepted; user info, an empty port,
//! and anything that isn't a plain host name or bracketed IPv6 address are
//! rejected. No dependency: a few string rules cover what browsers send in
//! `Origin`, `Referer` and `Host`.

use std::fmt;

/// The scheme of an origin. Only the two the app can be served over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
}

impl Scheme {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    const fn default_port(self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https => 443,
        }
    }

    const fn parse(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("http") {
            Some(Self::Http)
        } else if s.eq_ignore_ascii_case("https") {
            Some(Self::Https)
        } else {
            None
        }
    }
}

/// A normalized origin, e.g. `https://kauppa.example` or
/// `http://127.0.0.1:8080`. Equal origins have equal strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppOrigin(String);

impl AppOrigin {
    /// An exact origin, as in an `Origin` header or `KAUPPA_PUBLIC_ORIGIN`:
    /// `scheme://authority` with no path, query or fragment.
    pub fn parse(s: &str) -> Option<Self> {
        let (scheme, rest) = split_scheme(s)?;
        Self::from_authority(scheme, rest)
    }

    /// The origin part of a `Referer` URL: its scheme, host and port; the
    /// path, query and fragment are ignored.
    pub fn from_referer(s: &str) -> Option<Self> {
        let (scheme, rest) = split_scheme(s)?;
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        Self::from_authority(scheme, &rest[..end])
    }

    /// `scheme` plus an authority such as a `Host` header value
    /// (`host[:port]`).
    pub fn from_authority(scheme: Scheme, authority: &str) -> Option<Self> {
        let (host, port) = split_port(authority)?;
        let host = normalize_host(host)?;
        let origin = match port {
            Some(p) if p != scheme.default_port() => format!("{}://{host}:{p}", scheme.as_str()),
            _ => format!("{}://{host}", scheme.as_str()),
        };
        Some(Self(origin))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AppOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The value of `KAUPPA_PUBLIC_ORIGIN`, validated and normalized when the
/// settings load: a configuration error names the variable and the rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicOrigin(pub AppOrigin);

impl TryFrom<String> for PublicOrigin {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        AppOrigin::parse(s.trim()).map(Self).ok_or_else(|| {
            format!(
                "KAUPPA_PUBLIC_ORIGIN must be one origin such as https://kauppa.example \
                 (http or https, a host, an optional port, no path); got {s:?}"
            )
        })
    }
}

fn split_scheme(s: &str) -> Option<(Scheme, &str)> {
    let (scheme, rest) = s.split_once("://")?;
    Some((Scheme::parse(scheme)?, rest))
}

/// `host[:port]` → host and optional port. IPv6 hosts are bracketed.
fn split_port(authority: &str) -> Option<(&str, Option<u16>)> {
    let (host, port) = if authority.starts_with('[') {
        let close = authority.find(']')?;
        let (host, after) = authority.split_at(close + 1);
        if after.is_empty() {
            (host, None)
        } else {
            (host, Some(after.strip_prefix(':')?))
        }
    } else {
        match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        }
    };
    let port = match port {
        Some(p) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
            Some(p.parse::<u16>().ok()?)
        }
        Some(_) => return None,
        None => None,
    };
    Some((host, port))
}

/// A host name (letters, digits, `-`, `.`) or a bracketed IPv6 address,
/// lower-cased. User info (`@`) and anything else is rejected.
fn normalize_host(host: &str) -> Option<String> {
    let valid = if let Some(inner) = host.strip_prefix('[') {
        let inner = inner.strip_suffix(']')?;
        !inner.is_empty()
            && inner.contains(':')
            && inner
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.')
    } else {
        !host.is_empty()
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
    };
    valid.then(|| host.to_ascii_lowercase())
}
