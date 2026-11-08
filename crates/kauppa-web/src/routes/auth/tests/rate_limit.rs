//! MB-0026 P1–P6: failed logins and registrations are limited per client
//! and per account, before any database or Argon2 work (RR-0013). Time is
//! the limiter's fake clock (constraint 7: never `sleep`).

use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service, init_service};
use actix_web::web;
use chrono::Utc;
use sqlx::sqlite::SqliteConnectOptions;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::fmt::format::FmtSpan;

use kauppa_core::domain::InvitationState;
use kauppa_core::repo::InvitationRepo;

use super::{hidden_invite, invitation, register_request};
use crate::routes::auth::limiter::{
    AuthLimiter, ClientKey, FakeClock, LOGIN_PER_ACCOUNT, LOGIN_PER_CLIENT, MAX_KEYS,
    REGISTER_PER_CLIENT, TrustedProxies,
};
use crate::routes::test_support::{
    LogCapture, PASSWORD, add_default_proof, body_string, login_request, make_user, test_app,
    test_state_with_clock, wired_app,
};
use crate::state::AppState;
use crate::telemetry::RequestSpan;

const PAT: &str = "pat@example.com";
const GHOST: &str = "ghost@example.com";

fn addr(ip: &str) -> SocketAddr {
    SocketAddr::new(IpAddr::from_str(ip).expect("ip"), 50_000)
}

/// A failed login for `email` from `peer`.
fn bad_login(email: &str, peer: &str) -> TestRequest {
    TestRequest::post()
        .uri("/auth/login")
        .peer_addr(addr(peer))
        .set_form([("email", email), ("password", "not the password")])
}

/// The correct login for a user made by `make_user`, from `peer`.
fn good_login(email: &str, peer: &str) -> TestRequest {
    login_request(email).peer_addr(addr(peer))
}

fn retry_after<B>(resp: &actix_web::dev::ServiceResponse<B>) -> Option<u64> {
    resp.headers()
        .get(header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

/// `state`, but with a limiter that trusts `proxies`
/// (`KAUPPA_TRUSTED_PROXIES`).
fn with_trusted(
    state: &web::Data<AppState>,
    clock: &Arc<FakeClock>,
    proxies: &str,
) -> web::Data<AppState> {
    web::Data::new(AppState {
        limiter: Arc::new(AuthLimiter::new(
            clock.clone(),
            TrustedProxies::parse(proxies).expect("proxies"),
        )),
        ..AppState::clone(state)
    })
}

/// A second connection to the test database, to break it under the app.
async fn raw_pool(url: &str) -> sqlx::SqlitePool {
    sqlx::SqlitePool::connect_with(SqliteConnectOptions::from_str(url).expect("url"))
        .await
        .expect("raw pool")
}

/// Captures this thread's `tracing` output at every level (as in
/// `request_log`); with `spans`, also a line for each span opened.
fn capture_logs(
    spans: bool,
) -> (
    LogCapture,
    tracing::subscriber::DefaultGuard,
    tracing::Dispatch,
) {
    let idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let capture = LogCapture::default();
    let span_events = if spans { FmtSpan::NEW } else { FmtSpan::NONE };
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_span_events(span_events)
        .with_ansi(false)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    (capture, guard, idle)
}

/// P1: after `LOGIN_PER_CLIENT` failures from one client, its logins get
/// 429 with `Retry-After` until the window ends, even with the correct
/// password; other clients are unaffected; a throttled login does no
/// database work.
#[actix_web::test]
async fn per_client_login() {
    let (state, url, clock) = test_state_with_clock().await;
    let app = test_app!(state);
    make_user(&state, PAT).await;
    let attacker = "203.0.113.9";

    // Each failure for a different (unknown) email: only the client limit
    // can fire.
    for i in 0..LOGIN_PER_CLIENT.failures {
        let resp = call_service(
            &app,
            bad_login(&format!("guess{i}@example.com"), attacker).to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "failure {i}");
        assert_eq!(retry_after(&resp), None);
        clock.advance(Duration::from_secs(10));
    }

    // The correct password, from the same client: throttled.
    let resp = call_service(&app, good_login(PAT, attacker).to_request()).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    // The window started at the first failure, 100 s ago.
    assert_eq!(retry_after(&resp), Some(800));
    assert_eq!(resp.response().cookies().count(), 0, "not signed in");
    let body = body_string(resp).await;
    assert!(
        body.contains("Too many attempts. Try again in 14 minutes."),
        "{body}"
    );

    // Another client signs in.
    let resp = call_service(&app, good_login(PAT, "198.51.100.20").to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);

    // Throttled requests do no database work: with `users` gone, they
    // still get 429, not a server error. Nor do they extend the window.
    let raw = raw_pool(&url).await;
    sqlx::query("ALTER TABLE users RENAME TO users_away")
        .execute(&raw)
        .await
        .expect("rename users");
    for _ in 0..20 {
        let resp = call_service(&app, good_login(PAT, attacker).to_request()).await;
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }
    sqlx::query("ALTER TABLE users_away RENAME TO users")
        .execute(&raw)
        .await
        .expect("restore users");

    // One second before the window ends: still throttled, retry in 1 s.
    clock.advance(Duration::from_secs(799));
    let resp = call_service(&app, good_login(PAT, attacker).to_request()).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(retry_after(&resp), Some(1));
    let body = body_string(resp).await;
    assert!(
        body.contains("Too many attempts. Try again in 1 minute."),
        "{body}"
    );
    clock.advance(Duration::from_secs(1));
    let resp = call_service(&app, good_login(PAT, attacker).to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "window over");
}

/// P2: after `LOGIN_PER_ACCOUNT` failures for one email, from any mix of
/// clients, that email is throttled. An unknown email gets exactly the
/// same responses as an existing one.
#[actix_web::test]
async fn per_account_login() {
    let (state, url, clock) = test_state_with_clock().await;
    let app = test_app!(state);
    make_user(&state, PAT).await;

    // The same sequence for an existing and an unknown email, each failure
    // from a new client.
    let mut transcripts = Vec::new();
    for (n, email) in [(0, PAT), (1, GHOST)] {
        let mut transcript = Vec::new();
        for i in 0..LOGIN_PER_ACCOUNT.failures {
            let resp = call_service(
                &app,
                bad_login(email, &format!("192.0.2.{}", 10 * n + i)).to_request(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{email} {i}");
            transcript.push((resp.status(), retry_after(&resp), body_string(resp).await));
        }
        // From a client that never failed, with the correct password for
        // the existing account: throttled, as is any case of the email.
        for (variant, peer) in [
            (email.to_owned(), "198.51.100.1"),
            (email.to_uppercase(), "198.51.100.2"),
        ] {
            let resp = call_service(&app, good_login(&variant, peer).to_request()).await;
            assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS, "{variant}");
            assert_eq!(resp.response().cookies().count(), 0);
            transcript.push((resp.status(), retry_after(&resp), body_string(resp).await));
        }
        transcripts.push(transcript);
    }
    assert_eq!(
        transcripts[0], transcripts[1],
        "existing and unknown emails get the same answers"
    );
    assert_eq!(transcripts[0].last().map(|t| t.1), Some(Some(900)));

    // Other accounts are unaffected; the limiter never reads the database
    // (with `users` gone, an account at its limit still gets 429).
    let resp = call_service(
        &app,
        bad_login("other@example.com", "198.51.100.1").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let raw = raw_pool(&url).await;
    sqlx::query("ALTER TABLE users RENAME TO users_away")
        .execute(&raw)
        .await
        .expect("rename users");
    for email in [PAT, GHOST] {
        let resp = call_service(&app, good_login(email, "198.51.100.3").to_request()).await;
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS, "{email}");
    }
    sqlx::query("ALTER TABLE users_away RENAME TO users")
        .execute(&raw)
        .await
        .expect("restore users");

    clock.advance(LOGIN_PER_ACCOUNT.window);
    let resp = call_service(&app, good_login(PAT, "198.51.100.1").to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "window over");
}

/// P3: after `REGISTER_PER_CLIENT` failed registrations from one client,
/// whatever the failure, its registrations get 429 before the invitation
/// is checked or the password hashed; the page keeps the invitation.
#[actix_web::test]
async fn per_client_register() {
    let (state, url, clock) = test_state_with_clock().await;
    let app = test_app!(state);
    let attacker = "203.0.113.9";
    make_user(&state, "taken@example.com").await;
    let valid = invitation(&state).await;
    let unknown = "ab".repeat(32);

    // Every kind of failure counts: missing, malformed, unknown and used
    // invitations, a duplicate email, and a form that doesn't parse.
    let expected = [
        StatusCode::BAD_REQUEST,
        StatusCode::BAD_REQUEST,
        StatusCode::BAD_REQUEST,
        StatusCode::CONFLICT,
        StatusCode::BAD_REQUEST,
    ];
    for i in 0..REGISTER_PER_CLIENT.failures as usize {
        let req = match i % expected.len() {
            0 => register_request(None, "a@example.com"),
            1 => register_request(Some("not-a-token"), "a@example.com"),
            2 => register_request(Some(&unknown), "a@example.com"),
            3 => register_request(Some(&valid), "taken@example.com"),
            _ => TestRequest::post()
                .uri("/auth/register")
                .set_form([("email", "a@example.com")]),
        };
        let resp = call_service(&app, req.peer_addr(addr(attacker)).to_request()).await;
        assert_eq!(resp.status(), expected[i % expected.len()], "failure {i}");
        clock.advance(Duration::from_secs(60));
    }

    // A valid invitation, from the same client: throttled, without being
    // checked (with `invitations` gone, still 429) and without being used.
    let raw = raw_pool(&url).await;
    sqlx::query("ALTER TABLE invitations RENAME TO invitations_away")
        .execute(&raw)
        .await
        .expect("rename invitations");
    let resp = call_service(
        &app,
        register_request(Some(&valid), "new@example.com")
            .peer_addr(addr(attacker))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(retry_after(&resp), Some(3000));
    assert_eq!(
        resp.headers()
            .get(header::CACHE_CONTROL)
            .map(header::HeaderValue::as_bytes),
        Some(&b"no-store"[..]),
        "still private"
    );
    assert_eq!(resp.response().cookies().count(), 0);
    let body = body_string(resp).await;
    assert!(
        body.contains("Too many attempts. Try again in 50 minutes."),
        "{body}"
    );
    assert_eq!(
        hidden_invite(&body),
        Some(valid.clone()),
        "keeps the invite"
    );
    // So is a form that doesn't parse.
    let resp = call_service(
        &app,
        TestRequest::post()
            .uri("/auth/register")
            .peer_addr(addr(attacker))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    sqlx::query("ALTER TABLE invitations_away RENAME TO invitations")
        .execute(&raw)
        .await
        .expect("restore invitations");
    let listed = InvitationRepo::list(&state.pool).await.expect("list");
    assert_eq!(listed[0].state(Utc::now()), InvitationState::Pending);

    // Another client uses the invitation.
    let resp = call_service(
        &app,
        register_request(Some(&valid), "new@example.com")
            .peer_addr(addr("198.51.100.20"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
}

/// P4: successful logins neither count nor reset; entries expire; each map
/// holds at most `MAX_KEYS` entries, purging expired ones first and then
/// evicting the oldest.
#[actix_web::test]
async fn bounded_and_expiring() {
    let (state, _, clock) = test_state_with_clock().await;
    let app = test_app!(state);
    make_user(&state, PAT).await;
    let peer = "203.0.113.9";

    // Successes between failures don't count.
    for i in 0..LOGIN_PER_ACCOUNT.failures {
        let resp = call_service(&app, bad_login(PAT, peer).to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "failure {i}");
        if i + 1 < LOGIN_PER_ACCOUNT.failures {
            for _ in 0..3 {
                let resp = call_service(&app, good_login(PAT, peer).to_request()).await;
                assert_eq!(resp.status(), StatusCode::SEE_OTHER, "success after {i}");
            }
        }
    }
    // ... nor reset: the fifth failure reached the account limit.
    let resp = call_service(&app, good_login(PAT, peer).to_request()).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(state.limiter.lens(), [1, 1, 0]);

    // Entries expire, and are removed when next looked at.
    clock.advance(LOGIN_PER_CLIENT.window);
    let resp = call_service(&app, good_login(PAT, peer).to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(state.limiter.lens(), [0, 0, 0]);

    // A flood of distinct clients and emails, past the cap.
    let limiter = &state.limiter;
    let first = ClientKey::from(IpAddr::from_str("192.0.2.1").expect("ip"));
    let first_account = limiter.account_key("first@example.com");
    for _ in 0..LOGIN_PER_CLIENT.failures {
        limiter.record_login_failure(first, first_account);
    }
    assert!(limiter.check_login(first, first_account).is_some());
    clock.advance(Duration::from_secs(1));
    let flood = MAX_KEYS + 500;
    let client = |i: usize| {
        let i = u32::try_from(i).expect("small");
        ClientKey::from(IpAddr::from(std::net::Ipv4Addr::from(0x0a00_0000 + i)))
    };
    for i in 0..flood {
        limiter.record_login_failure(client(i), limiter.account_key(&format!("{i}@example.com")));
        limiter.record_register_failure(client(i));
    }
    assert_eq!(limiter.lens(), [MAX_KEYS; 3], "capped");
    // The oldest window went first.
    assert!(
        limiter.check_login(first, first_account).is_none(),
        "evicted"
    );
    assert!(
        limiter
            .check_login(client(flood - 1), limiter.account_key("x@example.com"))
            .is_none()
    );

    // At the cap, expired entries are purged before anything is evicted.
    clock.advance(REGISTER_PER_CLIENT.window);
    limiter.record_login_failure(first, first_account);
    limiter.record_register_failure(first);
    assert_eq!(limiter.lens(), [1, 1, 1], "all expired, all purged");
}

/// P5: the client is the TCP peer; `X-Forwarded-For` counts only from a
/// `KAUPPA_TRUSTED_PROXIES` peer, and then only its right-most untrusted
/// entry.
#[actix_web::test]
async fn forwarded_header_needs_trusted_proxy() {
    let (state, _, clock) = test_state_with_clock().await;
    let app = test_app!(state);
    make_user(&state, PAT).await;

    // Untrusted peer: a different spoofed header on every request doesn't
    // change its key.
    for i in 0..LOGIN_PER_CLIENT.failures {
        let resp = call_service(
            &app,
            bad_login(&format!("guess{i}@example.com"), "203.0.113.9")
                .insert_header(("X-Forwarded-For", format!("198.51.100.{i}")))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "failure {i}");
    }
    let resp = call_service(
        &app,
        good_login(PAT, "203.0.113.9")
            .insert_header(("X-Forwarded-For", "198.51.100.99"))
            .insert_header(("Forwarded", "for=198.51.100.98"))
            .insert_header(("X-Real-IP", "198.51.100.97"))
            .to_request(),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "spoof ignored"
    );
    let resp = call_service(&app, good_login(PAT, "203.0.113.10").to_request()).await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "the peer is the key");

    // Behind a trusted proxy, the forwarded client is the key.
    let state = with_trusted(&state, &clock, "172.30.0.0/24");
    let app = test_app!(state);
    let proxy = "172.30.0.2";
    let via = |xff: &str, email: &str| {
        bad_login(email, proxy).insert_header(("X-Forwarded-For", xff.to_owned()))
    };
    for i in 0..LOGIN_PER_CLIENT.failures {
        let resp = call_service(
            &app,
            via("198.51.100.7", &format!("guess{i}@example.com")).to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "failure {i}");
    }
    // That client is throttled, even when it prepends its own entry or
    // arrives through a second trusted hop ...
    for xff in [
        "198.51.100.7",
        "1.2.3.4, 198.51.100.7",
        "198.51.100.7, 172.30.0.9",
    ] {
        let resp = call_service(&app, via(xff, "next@example.com").to_request()).await;
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS, "{xff}");
    }
    // ... while other clients behind the same proxy are not.
    let resp = call_service(&app, via("198.51.100.8", "next@example.com").to_request()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // A client-chosen left-most entry is no way out.
    let resp = call_service(
        &app,
        via("198.51.100.8, 198.51.100.7", "next@example.com").to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // From the proxy without a usable header: the proxy's own (shared)
    // bucket, separate from its clients'.
    let resp = call_service(&app, bad_login("next@example.com", proxy).to_request()).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(state.limiter.lens()[0], 3, "198.51.100.7, .8 and the proxy");

    // A malformed `KAUPPA_TRUSTED_PROXIES` refuses to start.
    let load = |proxies: &str| {
        config::Config::builder()
            .set_override("bind", "127.0.0.1:0")
            .and_then(|b| b.set_override("database_url", "sqlite::memory:"))
            .and_then(|b| b.set_override("log_level", "info"))
            .and_then(|b| b.set_override("session_secret", "x".repeat(64)))
            .and_then(|b| b.set_override("cookie_secure", false))
            .and_then(|b| b.set_override("trusted_proxies", proxies))
            .expect("builder")
            .build()
            .expect("config")
            .try_deserialize::<crate::config::Settings>()
    };
    assert!(load("172.30.0.0/24, ::1").is_ok());
    assert_eq!(
        load("").expect("empty").trusted_proxies,
        TrustedProxies::default()
    );
    let err = load("172.30.0.0/33").expect_err("malformed").to_string();
    assert!(err.contains("KAUPPA_TRUSTED_PROXIES"), "{err}");
}

/// P6: throttling logs one WARN with the limiter and key kind, never the
/// email, the password or the address; the only other new lines are the
/// ones RR-0013 approves, and the request span's client IP is the
/// limiter's key, truncated.
#[actix_web::test]
async fn throttling_is_logged() {
    let (capture, guard, _idle) = capture_logs(false);
    let (state, _, clock) = test_state_with_clock().await;
    let app = init_service(
        wired_app!(state)
            .wrap(TracingLogger::<RequestSpan>::new())
            .wrap_fn(|mut req, srv| {
                add_default_proof(&mut req);
                actix_web::dev::Service::call(srv, req)
            }),
    )
    .await;
    make_user(&state, PAT).await;
    let peer = "203.0.113.9";

    // Account limit, then client limit, then register.
    for i in 0..LOGIN_PER_ACCOUNT.failures {
        let resp = call_service(&app, bad_login(PAT, peer).to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "failure {i}");
    }
    let resp = call_service(&app, good_login(PAT, peer).to_request()).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    for i in LOGIN_PER_ACCOUNT.failures..LOGIN_PER_CLIENT.failures {
        let resp = call_service(
            &app,
            bad_login(&format!("guess{i}@example.com"), peer).to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "failure {i}");
    }
    let resp = call_service(&app, good_login(GHOST, peer).to_request()).await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    for _ in 0..REGISTER_PER_CLIENT.failures {
        let resp = call_service(
            &app,
            register_request(None, PAT)
                .peer_addr(addr(peer))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
    let resp = call_service(
        &app,
        register_request(None, PAT)
            .peer_addr(addr(peer))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    let log = capture.text();
    let warns: Vec<&str> = log.lines().filter(|l| l.contains(" WARN ")).collect();
    assert_eq!(warns.len(), 3, "one line per throttled request: {log}");
    for (line, (limiter, kind, retry)) in warns.iter().zip([
        ("login", "account", 900),
        ("login", "client", 900),
        ("register", "client", 3600),
    ]) {
        assert!(
            line.contains(&format!(
                "auth attempt throttled limiter=\"{limiter}\" key_kind=\"{kind}\" retry_after_s={retry}"
            )),
            "{line}"
        );
        // The span's client IP is the key, truncated.
        assert!(line.contains("http.client_ip=203.0.113.0/24"), "{line}");
    }
    // Nothing else above DEBUG beyond the request spans' own lines (which
    // carry no message at INFO here), and no secret anywhere.
    for line in log.lines() {
        assert!(
            !line.contains(" INFO ") && !line.contains(" ERROR "),
            "no other line: {line}"
        );
    }
    for secret in [
        PAT,
        GHOST,
        "guess",
        PASSWORD,
        "not the password",
        peer,
        "203.0.113.9",
    ] {
        assert!(!log.contains(secret), "{secret} in the log: {log}");
    }

    // The approved operational lines: one capacity WARN per map per window,
    // and one trusted-proxy fallback WARN per process.
    let before = capture.text().len();
    let limiter = &state.limiter;
    for round in 0..2 {
        for i in 0..=MAX_KEYS {
            let key = ClientKey::from(IpAddr::from(std::net::Ipv4Addr::from(
                u32::try_from(i + round * (MAX_KEYS + 1)).expect("small"),
            )));
            limiter.record_register_failure(key);
        }
    }
    clock.advance(
        REGISTER_PER_CLIENT
            .window
            .saturating_sub(Duration::from_secs(1)),
    );
    limiter.record_register_failure(ClientKey::from(IpAddr::from([1, 1, 1, 1])));
    clock.advance(Duration::from_secs(1));
    // A new window: the whole map has expired, so nothing is evicted.
    limiter.record_register_failure(ClientKey::from(IpAddr::from([1, 1, 1, 2])));
    let trusted = with_trusted(&state, &clock, "127.0.0.1");
    for _ in 0..3 {
        trusted.limiter.client_key(
            Some(addr("127.0.0.1")),
            &actix_web::http::header::HeaderMap::new(),
        );
    }
    let log = capture.text();
    let added: Vec<&str> = log[before..].lines().collect();
    assert_eq!(
        added
            .iter()
            .filter(|l| l.contains(
                "WARN kauppa_web::routes::auth::limiter: auth limiter at capacity; evicting oldest limiter=\"register\" key_kind=\"client\""
            ))
            .count(),
        1,
        "{log}"
    );
    assert_eq!(
        added
            .iter()
            .filter(|l| l.contains(
                "WARN kauppa_web::routes::auth::limiter: forwarded-for missing from a trusted proxy"
            ))
            .count(),
        1,
        "{log}"
    );
    assert_eq!(added.len(), 2, "nothing else: {log}");
    drop(guard);
}

/// The request span records the limiter's client key, truncated, never a
/// client-chosen forwarded address (RR-0013 sub-answer 6).
#[actix_web::test]
async fn request_span_client_ip_is_the_truncated_key() {
    let (capture, _guard, _idle) = capture_logs(true);
    let (state, _, clock) = test_state_with_clock().await;
    for (state, peer, xff, expected) in [
        (
            state.clone(),
            "203.0.113.9",
            "198.51.100.7",
            "203.0.113.0/24",
        ),
        (
            state.clone(),
            "2001:db8:aa:bb:cc::1",
            "198.51.100.7",
            "2001:db8:aa::/48",
        ),
        (
            with_trusted(&state, &clock, "172.30.0.0/24"),
            "172.30.0.2",
            "198.51.100.7",
            "198.51.100.0/24",
        ),
    ] {
        let app = init_service(wired_app!(state).wrap(TracingLogger::<RequestSpan>::new())).await;
        let before = capture.text().len();
        let resp = call_service(
            &app,
            bad_login("x@example.com", peer)
                .insert_header(("X-Forwarded-For", xff))
                .insert_header((header::ORIGIN, "http://localhost:8080"))
                .insert_header((header::HOST, "localhost:8080"))
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let log = capture.text()[before..].to_owned();
        assert!(
            log.contains(&format!("http.client_ip={expected}")),
            "{peer}: {log}"
        );
        assert!(!log.contains(peer), "{peer}: full peer logged: {log}");
        if expected != "198.51.100.0/24" {
            assert!(!log.contains("198.51.100"), "{peer}: spoof logged: {log}");
        }
    }
}
