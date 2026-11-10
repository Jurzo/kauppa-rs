//! MB-0034: new passwords are length-checked on the server (RR-0022). The
//! check runs after the token-shape check and before `password::hash` and
//! `InvitationRepo::register`; login never applies it.

use std::net::SocketAddr;

use actix_web::http::{StatusCode, header};
use actix_web::test::{TestRequest, call_service};
use chrono::Utc;

use kauppa_core::domain::{InvitationState, PASSWORD_MAX_CHARS, PASSWORD_MIN_CHARS};
use kauppa_core::password;
use kauppa_core::repo::{InvitationRepo, UserRepo};

use super::{INVALID_INVITATION, hidden_invite, invitation};
use crate::routes::test_support::{LogCapture, body_string, source_files, test_app, test_state};
use crate::state::AppState;

/// The one length message, for too short and too long alike (RR-0022
/// sub-answer 6).
fn length_message() -> String {
    format!("Choose a password of {PASSWORD_MIN_CHARS} to {PASSWORD_MAX_CHARS} characters.")
}

/// A distinct client per request: these tests send more register failures
/// than MB-0026's per-client limit allows one client (RR-0013).
fn peer(n: u16) -> SocketAddr {
    let [hi, lo] = n.to_be_bytes();
    ([198, 51, hi, lo], 50_000).into()
}

/// The register form with `password`, from client `n`.
fn register_with(invite: &str, email: &str, password: &str, n: u16) -> TestRequest {
    TestRequest::post()
        .uri("/auth/register")
        .peer_addr(peer(n))
        .set_form([
            ("invite", invite),
            ("display_name", "Pat Q"),
            ("email", email),
            ("password", password),
        ])
}

fn login_with(email: &str, password: &str, n: u16) -> TestRequest {
    TestRequest::post()
        .uri("/auth/login")
        .peer_addr(peer(n))
        .set_form([("email", email), ("password", password)])
}

/// Captures this thread's `tracing` output at every level (as in
/// `request_log`).
fn capture_logs() -> (
    LogCapture,
    tracing::subscriber::DefaultGuard,
    tracing::Dispatch,
) {
    let idle = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let capture = LogCapture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .finish();
    let guard = tracing::subscriber::set_default(subscriber);
    tracing::callsite::rebuild_interest_cache();
    (capture, guard, idle)
}

async fn invitation_states(state: &AppState) -> Vec<InvitationState> {
    let now = Utc::now();
    InvitationRepo::list(&state.pool)
        .await
        .expect("list invitations")
        .iter()
        .map(|i| i.state(now))
        .collect()
}

async fn no_account(state: &AppState, email: &str) -> bool {
    UserRepo::find_by_email(&state.pool, email)
        .await
        .expect("lookup")
        .is_none()
}

/// Every out-of-bounds `passwords` entry, with a pending invitation, gets
/// the 400 length page, keeps the form, echoes nothing of the password and
/// logs nothing of it, creates nothing, and leaves the invitation usable.
async fn assert_rejected(passwords: &[(&str, String)]) {
    let (log, _guard, _idle) = capture_logs();
    let state = test_state().await;
    let app = test_app!(state);
    let token = invitation(&state).await;
    let email = "pat.q@example.com";

    for (n, (label, pw)) in (1u16..).zip(passwords) {
        let resp = call_service(&app, register_with(&token, email, pw, n).to_request()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{label}");
        assert_eq!(resp.response().cookies().count(), 0, "{label}: no session");
        assert_eq!(
            resp.headers()
                .get(header::CACHE_CONTROL)
                .map(header::HeaderValue::as_bytes),
            Some(&b"no-store"[..]),
            "{label}: private"
        );
        let body = body_string(resp).await;
        assert!(body.contains(&length_message()), "{label}: message: {body}");
        assert!(!body.contains(INVALID_INVITATION), "{label}");
        // The form is kept: the invitation, the name and the email, with
        // the password field empty.
        assert_eq!(
            hidden_invite(&body).as_deref(),
            Some(token.as_str()),
            "{label}"
        );
        assert!(body.contains("value=\"Pat Q\""), "{label}: display name");
        assert!(
            body.contains(&format!("value=\"{email}\"")),
            "{label}: email"
        );
        // (An empty or all-space password is in any page; the empty field
        // below covers those.)
        if !pw.trim().is_empty() {
            assert!(!body.contains(pw.as_str()), "{label}: password echoed");
        }
        let field = body
            .split("<input")
            .find(|i| i.contains("type=\"password\""))
            .expect("password field");
        assert!(!field.contains("value="), "{label}: password field empty");

        assert!(no_account(&state, email).await, "{label}: no account");
        assert_eq!(
            invitation_states(&state).await,
            [InvitationState::Pending],
            "{label}: invitation still pending"
        );
    }

    // Nothing of the password reaches the log: not the password, not its
    // length; at most a field-less line.
    let text = log.text();
    assert!(
        text.contains("registration rejected: password length"),
        "the field-less DEBUG line: {text}"
    );
    for (label, pw) in passwords {
        if !pw.trim().is_empty() {
            assert!(!text.contains(pw.as_str()), "{label}: password logged");
        }
    }
    for line in text.lines().filter(|l| l.contains("password")) {
        assert!(
            line.trim_end()
                .ends_with("registration rejected: password length"),
            "only a field-less line mentions the password: {line}"
        );
    }
    assert!(
        !text.contains("TooShort") && !text.contains("TooLong"),
        "{text}"
    );

    // The same invitation still works with a valid password.
    let resp = call_service(
        &app,
        register_with(&token, email, "a valid passphrase here", 999).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "invitation usable");
    assert!(!no_account(&state, email).await);
    assert_eq!(invitation_states(&state).await, [InvitationState::Used]);
}

/// P1: fewer than `PASSWORD_MIN_CHARS` characters is refused before the
/// hash and the transaction, counted in characters, not bytes.
#[actix_web::test]
async fn too_short_rejected() {
    let short = PASSWORD_MIN_CHARS - 1;
    assert_rejected(&[
        ("empty", String::new()),
        ("ascii", format!("Qx7{}", "k".repeat(short - 3))),
        // 14 characters but 28 bytes: a byte count would accept it.
        ("multi-byte", "ä".repeat(short)),
        // 14 characters but 56 bytes.
        ("emoji", "😀".repeat(short)),
        // Spaces count, nothing is trimmed: 14 spaces is 14 characters.
        ("spaces", " ".repeat(short)),
    ])
    .await;

    // The check reads no stored data (RR-0022 sub-answer 5): a well-formed
    // token gets the length page whatever its invitation's state, and a
    // malformed one the invalid-invitation page, as before.
    let state = test_state().await;
    let app = test_app!(state);
    let used = invitation(&state).await;
    let resp = call_service(
        &app,
        register_with(&used, "first@example.com", "a valid passphrase here", 1).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    let unknown = "ab".repeat(32);
    for (n, (label, token)) in (2u16..).zip([("used", used.as_str()), ("unknown", &unknown)]) {
        let resp = call_service(
            &app,
            register_with(token, "second@example.com", "short", n).to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{label}");
        let body = body_string(resp).await;
        assert!(body.contains(&length_message()), "{label}: {body}");
    }
    let resp = call_service(
        &app,
        register_with("not-a-token", "second@example.com", "short", 9).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = body_string(resp).await;
    assert!(body.contains(INVALID_INVITATION), "malformed: {body}");
    assert!(!body.contains(&length_message()), "malformed: {body}");
    assert!(no_account(&state, "second@example.com").await);
}

/// P2: more than `PASSWORD_MAX_CHARS` characters is refused the same way,
/// before any Argon2 work.
#[actix_web::test]
async fn too_long_rejected() {
    let long = PASSWORD_MAX_CHARS + 1;
    assert_rejected(&[
        ("ascii", format!("Lz8{}", "w".repeat(long - 3))),
        ("multi-byte", "ö".repeat(long)),
        ("emoji", "🔑".repeat(long)),
        // Near actix-web's 32 KiB form limit.
        ("large", "H".repeat(20_000)),
    ])
    .await;

    // Before cost (RR-0022 constraint 4): in `register`, the length check
    // follows the token-shape check and precedes the hash and the
    // transaction.
    let (_, source) = source_files()
        .into_iter()
        .find(|(path, _)| path == "routes/auth/mod.rs")
        .expect("routes/auth/mod.rs");
    let start = source.find("async fn register(").expect("register fn");
    let body = &source[start..];
    let body = &body[..body.find("\nfn ").expect("end of register")];
    let at = |needle: &str| {
        body.find(needle)
            .unwrap_or_else(|| panic!("`{needle}` in register"))
    };
    let shape = at("token::is_well_formed(");
    let length = at("check_new_password_length(");
    let hash = at("password::hash(");
    let tx = at("InvitationRepo::register(");
    assert!(shape < length, "length after the token shape");
    assert!(length < hash, "length before the hash");
    assert!(hash < tx, "hash before the transaction");
    assert_eq!(
        body.matches("check_new_password_length(").count(),
        1,
        "one check"
    );
}

/// P3: exactly `PASSWORD_MIN_CHARS` and `PASSWORD_MAX_CHARS` characters are
/// accepted, counted as Unicode scalar values on the string as submitted,
/// and the account signs in with that exact password.
#[actix_web::test]
async fn bounds_accepted() {
    let state = test_state().await;
    let app = test_app!(state);
    let min = PASSWORD_MIN_CHARS;
    let max = PASSWORD_MAX_CHARS;
    let cases = [
        ("min ascii", "m".repeat(min)),
        ("max ascii", "M".repeat(max)),
        ("min multi-byte", "ä".repeat(min)),
        ("max emoji", "😀".repeat(max)),
        // Not trimmed: 11 letters and 4 spaces are 15 characters.
        ("min with spaces", format!("  {}  ", "s".repeat(min - 4))),
        // Not normalized: "a" + combining diaeresis is two characters.
        (
            "min decomposed",
            format!("{}x", "a\u{308}".repeat((min - 1) / 2)),
        ),
    ];
    for (n, (label, pw)) in (1u16..).zip(&cases) {
        assert_eq!(
            pw.chars().count(),
            if label.starts_with("min") { min } else { max }
        );
        let email = format!("user{n}@example.com");
        let token = invitation(&state).await;
        let resp = call_service(&app, register_with(&token, &email, pw, n).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{label}");
        assert!(!no_account(&state, &email).await, "{label}: account");

        let resp = call_service(&app, login_with(&email, pw, n).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{label}: login");
    }
}

/// P4: login never applies the rule: accounts whose passwords are outside
/// the bounds still sign in, and a short wrong password gets the login
/// error, not the length message.
#[actix_web::test]
async fn login_unaffected() {
    let state = test_state().await;
    let app = test_app!(state);
    let short = "pw";
    let long = "L".repeat(PASSWORD_MAX_CHARS * 2);
    for (n, (email, pw)) in (1u16..).zip([
        ("short@example.com", short),
        ("long@example.com", long.as_str()),
    ]) {
        let hash = password::hash(pw).expect("hash");
        UserRepo::create(&state.pool, email, "Old Account", &hash)
            .await
            .expect("create user");
        let resp = call_service(&app, login_with(email, pw, n).to_request()).await;
        assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{email}");
    }
    for (n, email) in (10u16..).zip(["short@example.com", "nobody@example.com"]) {
        let resp = call_service(&app, login_with(email, "x", n).to_request()).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{email}");
        let body = body_string(resp).await;
        assert!(!body.contains(&length_message()), "{email}: {body}");
    }
}

/// P5: the register page's `minlength` is `PASSWORD_MIN_CHARS`, the page
/// has no `maxlength`, and the template holds no numeric literal for
/// either.
#[actix_web::test]
async fn form_matches_rule() {
    let state = test_state().await;
    let app = test_app!(state);
    let token = invitation(&state).await;
    let minlength = format!("minlength=\"{PASSWORD_MIN_CHARS}\"");

    let resp = call_service(
        &app,
        TestRequest::get()
            .uri(&format!("/auth/register?invite={token}"))
            .to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let form = body_string(resp).await;
    // The 400 length page carries the same form.
    let resp = call_service(
        &app,
        register_with(&token, "pat@example.com", "short", 1).to_request(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let rejected = body_string(resp).await;
    for (label, page) in [("form", &form), ("rejected", &rejected)] {
        let field = page
            .split("<input")
            .find(|i| i.contains("type=\"password\""))
            .unwrap_or_else(|| panic!("{label}: password field"));
        assert!(field.contains(&minlength), "{label}: {field}");
        assert!(!page.contains("maxlength"), "{label}: no maxlength");
    }

    let template = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/auth/register.hbs"),
    )
    .expect("read register.hbs");
    assert!(
        !template.contains("maxlength"),
        "no maxlength in the template"
    );
    for value in template.split("minlength=\"").skip(1) {
        assert!(value.starts_with("{{"), "minlength from the view: {value}");
    }
}
