//! Unit tests: origin parsing and normalization, `KAUPPA_PUBLIC_ORIGIN`
//! validation, and the decision table of `csrf::check` (RR-0010).

use actix_web::http::Uri;
use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue};

use crate::csrf::{AppOrigin, OriginPolicy, PublicOrigin, Rejection, Scheme, check};

fn origin(s: &str) -> Option<String> {
    AppOrigin::parse(s).map(|o| o.as_str().to_owned())
}

#[test]
fn origins_are_normalized() {
    for (input, expected) in [
        ("http://localhost:8080", "http://localhost:8080"),
        ("HTTPS://Kauppa.Example", "https://kauppa.example"),
        ("https://kauppa.example:443", "https://kauppa.example"),
        ("http://kauppa.example:80", "http://kauppa.example"),
        ("http://kauppa.example:443", "http://kauppa.example:443"),
        ("https://kauppa.example:80", "https://kauppa.example:80"),
        ("http://127.0.0.1:18123", "http://127.0.0.1:18123"),
        ("http://[::1]:8080", "http://[::1]:8080"),
        ("http://[FE80::1]", "http://[fe80::1]"),
    ] {
        assert_eq!(origin(input).as_deref(), Some(expected), "{input}");
    }
}

#[test]
fn malformed_origins_are_rejected() {
    for input in [
        "",
        "null",
        "localhost:8080",
        "ftp://kauppa.example",
        "http://",
        "http://kauppa.example/",
        "http://kauppa.example/path",
        "http://kauppa.example?q",
        "http://kauppa.example:",
        "http://kauppa.example:99999",
        "http://kauppa.example:8o",
        "http://user@kauppa.example",
        "http://kauppa example",
        "http://kauppa.example\n",
        "http://[::1",
        "http://[]",
        "http://[::1]x",
    ] {
        assert_eq!(origin(input), None, "{input:?}");
    }
}

#[test]
fn referer_origin_ignores_path_query_and_fragment() {
    for (input, expected) in [
        (
            "http://localhost:8080/lists/1?x=y#z",
            Some("http://localhost:8080"),
        ),
        (
            "https://Kauppa.Example:443/",
            Some("https://kauppa.example"),
        ),
        ("http://localhost:8080?x", Some("http://localhost:8080")),
        ("http://localhost:8080#x", Some("http://localhost:8080")),
        ("http://localhost:8080", Some("http://localhost:8080")),
        ("/lists", None),
        ("http://user:pw@localhost:8080/", None),
        ("javascript:alert(1)", None),
    ] {
        let got = AppOrigin::from_referer(input);
        assert_eq!(got.as_ref().map(AppOrigin::as_str), expected, "{input}");
    }
}

/// `KAUPPA_PUBLIC_ORIGIN` takes exactly one origin; the error names the
/// variable.
#[test]
fn public_origin_is_validated() {
    let ok = PublicOrigin::try_from(" HTTPS://Kauppa.Example:443 ".to_owned());
    assert_eq!(
        ok.map(|p| p.0.as_str().to_owned()),
        Ok("https://kauppa.example".to_owned())
    );
    for bad in [
        "https://kauppa.example/",
        "https://kauppa.example/app",
        "https://a.example,https://b.example",
        "kauppa.example",
    ] {
        let err = PublicOrigin::try_from(bad.to_owned()).expect_err(bad);
        assert!(err.contains("KAUPPA_PUBLIC_ORIGIN"), "{bad}: {err}");
    }
}

#[test]
fn policy_follows_settings() {
    let public = PublicOrigin::try_from("https://kauppa.example".to_owned()).expect("origin");
    assert_eq!(
        OriginPolicy::new(Some(public.clone()), false),
        OriginPolicy::Fixed(public.0)
    );
    assert_eq!(
        OriginPolicy::new(None, true),
        OriginPolicy::SameHost(Scheme::Https)
    );
    assert_eq!(
        OriginPolicy::new(None, false),
        OriginPolicy::SameHost(Scheme::Http)
    );
}

fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for &(name, value) in pairs {
        map.append(
            HeaderName::from_bytes(name.as_bytes()).expect("name"),
            HeaderValue::from_str(value).expect("value"),
        );
    }
    map
}

fn verdict(policy: &OriginPolicy, pairs: &[(&str, &str)]) -> Result<(), Rejection> {
    check(policy, &headers(pairs), &Uri::from_static("/lists")).map_err(|(r, _)| r)
}

const HOST: (&str, &str) = ("host", "localhost:8080");

#[test]
fn same_host_acceptances() {
    let policy = OriginPolicy::SameHost(Scheme::Http);
    let ok: &[&[(&str, &str)]] = &[
        &[HOST, ("origin", "http://localhost:8080")],
        &[HOST, ("origin", "http://LOCALHOST:8080")],
        &[
            ("host", "LocalHost:8080"),
            ("origin", "http://localhost:8080"),
        ],
        &[("host", "kauppa.lan"), ("origin", "http://kauppa.lan:80")],
        &[HOST, ("referer", "http://localhost:8080/lists")],
        &[
            HOST,
            ("sec-fetch-site", "same-origin"),
            ("origin", "http://localhost:8080"),
        ],
        &[
            HOST,
            ("sec-fetch-site", "none"),
            ("origin", "http://localhost:8080"),
        ],
        // Origin wins over Referer when both are present.
        &[
            HOST,
            ("origin", "http://localhost:8080"),
            ("referer", "http://evil.example/"),
        ],
    ];
    for pairs in ok {
        assert_eq!(verdict(&policy, pairs), Ok(()), "{pairs:?}");
    }
}

#[test]
fn same_host_rejections() {
    let policy = OriginPolicy::SameHost(Scheme::Http);
    let evil = Some(AppOrigin::parse("http://evil.example").expect("origin"));
    let rejected: &[(&[(&str, &str)], Rejection)] = &[
        (
            &[
                HOST,
                ("sec-fetch-site", "cross-site"),
                ("origin", "http://localhost:8080"),
            ],
            Rejection::FetchMetadata,
        ),
        (
            &[
                HOST,
                ("sec-fetch-site", "Same-Site"),
                ("origin", "http://localhost:8080"),
            ],
            Rejection::FetchMetadata,
        ),
        (
            &[HOST, ("origin", "http://evil.example")],
            Rejection::OriginMismatch { got: evil.clone() },
        ),
        (
            &[HOST, ("origin", "https://localhost:8080")],
            Rejection::OriginMismatch {
                got: AppOrigin::parse("https://localhost:8080"),
            },
        ),
        (
            &[HOST, ("origin", "http://localhost:8081")],
            Rejection::OriginMismatch {
                got: AppOrigin::parse("http://localhost:8081"),
            },
        ),
        (
            &[HOST, ("origin", "http://sub.localhost:8080")],
            Rejection::OriginMismatch {
                got: AppOrigin::parse("http://sub.localhost:8080"),
            },
        ),
        (&[HOST, ("origin", "null")], Rejection::NullOrigin),
        (
            &[HOST, ("origin", "garbage")],
            Rejection::OriginMismatch { got: None },
        ),
        (
            &[
                HOST,
                ("origin", "http://localhost:8080"),
                ("origin", "http://localhost:8080"),
            ],
            Rejection::OriginMismatch { got: None },
        ),
        // Origin decides when present: a matching Referer doesn't rescue it.
        (
            &[
                HOST,
                ("origin", "http://evil.example"),
                ("referer", "http://localhost:8080/"),
            ],
            Rejection::OriginMismatch { got: evil.clone() },
        ),
        (
            &[HOST, ("referer", "http://evil.example/page")],
            Rejection::RefererMismatch { got: evil },
        ),
        (
            &[HOST, ("referer", "/relative")],
            Rejection::RefererMismatch { got: None },
        ),
        (&[HOST], Rejection::MissingProof),
        // Fetch Metadata never accepts on its own.
        (
            &[HOST, ("sec-fetch-site", "same-origin")],
            Rejection::MissingProof,
        ),
        (
            &[("origin", "http://localhost:8080")],
            Rejection::UnknownAppOrigin,
        ),
        (
            &[("host", "bad host"), ("origin", "http://localhost:8080")],
            Rejection::UnknownAppOrigin,
        ),
    ];
    for (pairs, expected) in rejected {
        assert_eq!(verdict(&policy, pairs).as_ref(), Err(expected), "{pairs:?}");
    }
}

#[test]
fn same_host_scheme_comes_from_the_policy() {
    let policy = OriginPolicy::SameHost(Scheme::Https);
    let pairs = [
        ("host", "kauppa.example"),
        ("origin", "https://kauppa.example"),
    ];
    assert_eq!(verdict(&policy, &pairs), Ok(()));
    let pairs = [
        ("host", "kauppa.example"),
        ("origin", "http://kauppa.example"),
    ];
    assert!(matches!(
        verdict(&policy, &pairs),
        Err(Rejection::OriginMismatch { .. })
    ));
}

/// A configured public origin is compared exactly; `Host` and
/// `X-Forwarded-Host` are never consulted.
#[test]
fn fixed_origin_ignores_host() {
    let public = AppOrigin::parse("https://kauppa.example").expect("origin");
    let policy = OriginPolicy::Fixed(public);
    let ok = [
        ("host", "127.0.0.1:8080"),
        ("origin", "https://kauppa.example"),
    ];
    assert_eq!(verdict(&policy, &ok), Ok(()));
    let ok = [("referer", "https://kauppa.example/lists")];
    assert_eq!(verdict(&policy, &ok), Ok(()));
    let host_match = [
        ("host", "127.0.0.1:8080"),
        ("origin", "http://127.0.0.1:8080"),
    ];
    assert!(matches!(
        verdict(&policy, &host_match),
        Err(Rejection::OriginMismatch { .. })
    ));
    let forwarded = [
        ("host", "kauppa.example"),
        ("x-forwarded-host", "evil.example"),
        ("origin", "https://evil.example"),
    ];
    assert!(matches!(
        verdict(&policy, &forwarded),
        Err(Rejection::OriginMismatch { .. })
    ));
}

/// Every reason has a distinct log label.
#[test]
fn reasons_are_distinct() {
    let reasons = [
        Rejection::FetchMetadata,
        Rejection::NullOrigin,
        Rejection::OriginMismatch { got: None },
        Rejection::RefererMismatch { got: None },
        Rejection::MissingProof,
        Rejection::UnknownAppOrigin,
    ]
    .map(|r| r.reason());
    let unique: std::collections::BTreeSet<_> = reasons.iter().collect();
    assert_eq!(unique.len(), reasons.len(), "{reasons:?}");
}
