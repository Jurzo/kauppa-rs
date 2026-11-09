//! Unit tests for the limiter's parts: `KAUPPA_TRUSTED_PROXIES` parsing,
//! client-key resolution and the fixed windows (RR-0013). The route-level
//! properties are in `routes::auth::tests::rate_limit`.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use actix_web::http::header::{HeaderMap, HeaderName, HeaderValue};

use super::{
    AuthLimiter, ClientKey, FakeClock, LOGIN_PER_ACCOUNT, LOGIN_PER_CLIENT, MAX_KEYS,
    REGISTER_PER_CLIENT, Resolved, Throttled, TrustedProxies,
};

#[allow(clippy::unnecessary_wraps)] // `resolve` takes the peer as an `Option`.
fn peer(ip: &str) -> Option<SocketAddr> {
    Some(SocketAddr::new(ip.parse().unwrap(), 40_000))
}

fn xff(lines: &[&str]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for line in lines {
        headers.append(
            HeaderName::from_static("x-forwarded-for"),
            HeaderValue::from_str(line).unwrap(),
        );
    }
    headers
}

fn key(ip: &str) -> ClientKey {
    ClientKey::from(ip.parse::<IpAddr>().unwrap())
}

#[test]
fn trusted_proxies_parse() {
    assert!(TrustedProxies::parse("").unwrap().is_empty());
    assert!(TrustedProxies::parse("  ").unwrap().is_empty());
    for ok in [
        "127.0.0.1",
        "172.30.0.0/24",
        "172.30.0.0/24, 10.0.0.1",
        "::1",
        "fd00::/8",
        "0.0.0.0/0",
        "::ffff:10.0.0.1",
        "10.1.2.3/32",
        "::/128",
    ] {
        assert!(TrustedProxies::parse(ok).is_ok(), "{ok}");
    }
    for bad in [
        "localhost",
        "172.30.0.0/33",
        "::/129",
        "172.30.0.0/",
        "172.30.0.0/+8",
        "172.30.0.0/-1",
        "10.0.0.1,",
        "10.0.0.1,,10.0.0.2",
        "10.0.0.1:80",
        "300.0.0.1",
    ] {
        let err = TrustedProxies::parse(bad).expect_err(bad);
        assert!(err.starts_with("KAUPPA_TRUSTED_PROXIES: "), "{bad}: {err}");
    }
}

#[test]
fn untrusted_peer_is_the_client() {
    let none = TrustedProxies::default();
    let spoof = xff(&["198.51.100.7"]);
    assert_eq!(
        none.resolve(peer("203.0.113.9"), &spoof),
        Resolved::Client(key("203.0.113.9"))
    );
    let other = TrustedProxies::parse("10.0.0.0/8").unwrap();
    assert_eq!(
        other.resolve(peer("203.0.113.9"), &spoof),
        Resolved::Client(key("203.0.113.9"))
    );
    assert_eq!(
        none.resolve(None, &spoof),
        Resolved::Client(ClientKey::UnknownPeer)
    );
}

#[test]
fn trusted_peer_takes_right_most_untrusted_entry() {
    let trusted = TrustedProxies::parse("172.30.0.0/24, 10.0.0.5").unwrap();
    let caddy = peer("172.30.0.2");
    let cases: [(&[&str], &str); 6] = [
        (&["198.51.100.7"], "198.51.100.7"),
        // A client-sent entry on the left never wins over the one the proxy
        // appended.
        (&["1.1.1.1, 198.51.100.7"], "198.51.100.7"),
        (&["1.1.1.1", "198.51.100.7"], "198.51.100.7"),
        // Trusted hops on the right are skipped.
        (&["198.51.100.7, 10.0.0.5, 172.30.0.9"], "198.51.100.7"),
        (&["198.51.100.7", "10.0.0.5"], "198.51.100.7"),
        (&[" 2001:db8::1 "], "2001:db8::1"),
    ];
    for (lines, client) in cases {
        assert_eq!(
            trusted.resolve(caddy, &xff(lines)),
            Resolved::Client(key(client)),
            "{lines:?}"
        );
    }
    // An IPv4-mapped peer is matched as IPv4.
    assert_eq!(
        trusted.resolve(peer("::ffff:172.30.0.2"), &xff(&["198.51.100.7"])),
        Resolved::Client(key("198.51.100.7"))
    );
}

#[test]
fn trusted_peer_without_usable_header_is_the_shared_bucket() {
    let trusted = TrustedProxies::parse("172.30.0.0/24").unwrap();
    let caddy = peer("172.30.0.2");
    let cases: [&[&str]; 6] = [
        &[],
        &[""],
        &["not-an-address"],
        &["198.51.100.7, garbage"],
        &["198.51.100.7:4321"],
        &["172.30.0.7, 172.30.0.8"],
    ];
    for lines in cases {
        assert_eq!(
            trusted.resolve(caddy, &xff(lines)),
            Resolved::ProxyFallback(key("172.30.0.2")),
            "{lines:?}"
        );
    }
}

#[test]
fn keys_normalize_and_truncate() {
    assert_eq!(key("::ffff:203.0.113.9"), key("203.0.113.9"));
    // One IPv6 /64 is one client.
    assert_eq!(key("2001:db8:1:2:aaaa::1"), key("2001:db8:1:2:bbbb::2"));
    assert_ne!(key("2001:db8:1:2::1"), key("2001:db8:1:3::1"));

    assert_eq!(key("203.0.113.9").truncated().to_string(), "203.0.113.0/24");
    assert_eq!(
        key("2001:db8:1:2:aaaa::1").truncated().to_string(),
        "2001:db8:1::/48"
    );
    assert_eq!(ClientKey::UnknownPeer.truncated().to_string(), "");
}

#[test]
fn constants_are_rr_0013s() {
    assert_eq!(
        (LOGIN_PER_CLIENT.failures, LOGIN_PER_CLIENT.window),
        (10, Duration::from_secs(900))
    );
    assert_eq!(
        (LOGIN_PER_ACCOUNT.failures, LOGIN_PER_ACCOUNT.window),
        (5, Duration::from_secs(900))
    );
    assert_eq!(
        (REGISTER_PER_CLIENT.failures, REGISTER_PER_CLIENT.window),
        (10, Duration::from_secs(3600))
    );
    assert_eq!(MAX_KEYS, 10_000);
}

#[test]
fn fixed_window_from_first_failure() {
    let clock = Arc::new(FakeClock::default());
    let limiter = AuthLimiter::new(clock.clone(), TrustedProxies::default());
    let client = key("203.0.113.9");

    for _ in 0..REGISTER_PER_CLIENT.failures {
        assert_eq!(limiter.check_register(client), None);
        limiter.record_register_failure(client);
        clock.advance(Duration::from_secs(60));
    }
    // Ten minutes in: 50 minutes left of the hour.
    assert_eq!(
        limiter.check_register(client),
        Some(Throttled {
            retry_after_s: 3000
        })
    );
    clock.advance(Duration::from_millis(500));
    let t = limiter.check_register(client).unwrap();
    assert_eq!(t.retry_after_s, 3000, "rounded up");
    assert_eq!(t.minutes(), 50);
    // Checks while throttled don't extend the window.
    clock.advance(Duration::from_millis(2_999_500));
    assert_eq!(limiter.check_register(client), None, "window over");
    assert_eq!(limiter.lens(), [0, 0, 0], "expired entry purged on check");
}

#[test]
fn account_key_normalizes_email() {
    let limiter = AuthLimiter::new(Arc::new(FakeClock::default()), TrustedProxies::default());
    assert_eq!(
        limiter.account_key(" Pat@Example.COM "),
        limiter.account_key("pat@example.com")
    );
    assert_ne!(
        limiter.account_key("pat@example.com"),
        limiter.account_key("pat2@example.com")
    );
}

#[test]
fn unknown_peer_is_one_bucket() {
    let limiter = AuthLimiter::new(Arc::new(FakeClock::default()), TrustedProxies::default());
    let a = limiter.client_key(None, &xff(&["198.51.100.7"]));
    let b = limiter.client_key(None, &HeaderMap::new());
    assert_eq!(a, ClientKey::UnknownPeer);
    assert_eq!(a, b);
    assert_ne!(
        limiter.client_key(peer("127.0.0.1"), &HeaderMap::new()),
        ClientKey::UnknownPeer
    );
    assert_eq!(
        limiter.client_key(peer("127.0.0.1"), &HeaderMap::new()),
        ClientKey::V4(Ipv4Addr::LOCALHOST)
    );
}
