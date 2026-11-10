//! Authentication endpoints.
//!
//! Password hashing goes through `kauppa_core::password` — never hash in a
//! handler. Session lifetime is controlled by `actix-identity` on top of
//! `actix-session`; `Identity::login` writes the session cookie.
//!
//! Registration is invite-only (MB-0023, RR-0012): an account is created
//! only by using a pending invitation, in the same transaction
//! (`InvitationRepo::register`). The invitation link is
//! `/auth/register?invite=<token>`; the page carries the token on in a
//! hidden field, so it never appears in a POST URL.
//!
//! Failed logins and registrations are rate-limited (MB-0026, RR-0013):
//! both handlers consult [`limiter::AuthLimiter`] before any database
//! lookup, invitation check or Argon2 call, and a throttled request does
//! no other work.
//!
//! New passwords are length-checked (MB-0034, RR-0022) by the `kauppa-core`
//! rule, after the token shape and before the hash; login never applies it.

use std::fmt;
use std::sync::OnceLock;

use actix_identity::Identity;
use actix_web::http::StatusCode;
use actix_web::http::header::{self, HeaderValue};
use actix_web::{HttpMessage, HttpRequest, HttpResponse, get, post, web};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use kauppa_core::domain::{Language, PASSWORD_MIN_CHARS, check_new_password_length};
use kauppa_core::repo::{InvitationRepo, UserRepo};
use kauppa_core::{CoreError, CoreResult, password, token};

use crate::error::{WebError, WebResult};
use crate::i18n::messages::Message;
use crate::i18n::{COOKIE, Ui, VARY};
use crate::state::AppState;

use self::limiter::Throttled;

pub mod limiter;

#[cfg(test)]
pub mod tests;

#[derive(Debug, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

/// The register form. `invite` is the hidden field; a missing one is the
/// same as an invalid one.
#[derive(Deserialize)]
pub struct Registration {
    #[serde(default)]
    pub invite: String,
    pub email: String,
    pub display_name: String,
    pub password: String,
}

/// Never shows the token, the password or the email.
impl fmt::Debug for Registration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registration").finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
pub struct InviteQuery {
    #[serde(default)]
    invite: Option<String>,
}

/// Never shows the token.
impl fmt::Debug for InviteQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InviteQuery").finish_non_exhaustive()
    }
}

/// The register page (`auth/register`): the form when it has a token,
/// otherwise "registration needs an invitation". `password_min` is the
/// form's `minlength`, which [`render_register`] sets from
/// `PASSWORD_MIN_CHARS` (RR-0022 sub-answer 7).
#[derive(Debug, Default, Serialize)]
struct RegisterView<'a> {
    invite: Option<&'a str>,
    error: Option<&'a str>,
    display_name: &'a str,
    email: &'a str,
    password_min: usize,
}

/// The password the dummy hash is made from. Its value doesn't matter: a
/// match changes nothing, since an unknown email always gets the 401.
const DUMMY_PASSWORD: &str = "kauppa-rs dummy password for unknown emails";

static DUMMY_HASH: OnceLock<String> = OnceLock::new();

/// A fixed PHC string, made once per process with `password::hash`, that
/// login verifies against for an unknown email, so every login attempt
/// costs one Argon2 verify (RR-0013 sub-answer 7). `main.rs` makes it at
/// startup.
pub fn dummy_hash() -> CoreResult<&'static str> {
    if let Some(hash) = DUMMY_HASH.get() {
        return Ok(hash);
    }
    let hash = password::hash(DUMMY_PASSWORD)?;
    Ok(DUMMY_HASH.get_or_init(|| hash))
}

#[get("/login")]
pub async fn login_form(state: web::Data<AppState>, ui: Ui) -> WebResult<HttpResponse> {
    let body = ui.render(&state, "auth/login", &json!({ "error": null }))?;
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .insert_header(VARY)
        .body(body))
}

#[post("/login")]
pub async fn login_submit(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    form: web::Form<Credentials>,
) -> WebResult<HttpResponse> {
    // Limit before cost (RR-0013 constraint 1): client, then account, before
    // the lookup and Argon2. Unknown and known emails are keyed alike.
    let client = state.limiter.client_key(req.peer_addr(), req.headers());
    let account = state.limiter.account_key(&form.email);
    if let Some(throttled) = state.limiter.check_login(client, account) {
        return render_login_throttled(&state, &ui, throttled);
    }

    let found = state.pool.find_by_email(&form.email).await?;
    let user = if let Some((user, stored_hash)) = found {
        password::verify(&form.password, &stored_hash)?.then_some(user)
    } else {
        // The same Argon2 cost as a wrong password, and the same answer.
        password::verify(&form.password, dummy_hash()?)?;
        None
    };
    let Some(user) = user else {
        state.limiter.record_login_failure(client, account);
        return render_login_error(&state, &ui, &ui.message(Message::InvalidLogin)?);
    };

    Identity::login(&req.extensions(), user.id.to_string())
        .map_err(|e| WebError::Internal(format!("session login failed: {e}")))?;

    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/lists"))
        .finish())
}

/// The 429 page: the same text whichever limiter fired (RR-0013
/// constraint 4).
fn render_login_throttled(
    state: &AppState,
    ui: &Ui,
    throttled: Throttled,
) -> WebResult<HttpResponse> {
    let body = ui.render(
        state,
        "auth/login",
        &json!({ "error": too_many_attempts(ui, throttled)? }),
    )?;
    Ok(HttpResponse::TooManyRequests()
        .insert_header((header::RETRY_AFTER, throttled.retry_after_s))
        .content_type("text/html")
        .insert_header(VARY)
        .body(body))
}

/// "Too many attempts. Try again in N minutes." (RR-0013 sub-answer 1), in
/// the request's language.
fn too_many_attempts(ui: &Ui, throttled: Throttled) -> WebResult<String> {
    ui.message(Message::TooManyAttempts {
        minutes: throttled.minutes(),
    })
}

fn render_login_error(state: &AppState, ui: &Ui, msg: &str) -> WebResult<HttpResponse> {
    let body = ui.render(state, "auth/login", &json!({ "error": msg }))?;
    Ok(HttpResponse::Unauthorized()
        .content_type("text/html")
        .insert_header(VARY)
        .body(body))
}

/// The register page. With `?invite=<token>` it renders the form with the
/// token in a hidden field, without checking it; without one (or with an
/// empty one) it says registration needs an invitation.
#[get("/register")]
pub async fn register_form(
    state: web::Data<AppState>,
    ui: Ui,
    query: Result<web::Query<InviteQuery>, actix_web::Error>,
) -> HttpResponse {
    private(query.and_then(|query| {
        let invite = query.invite.as_deref().filter(|t| !t.is_empty());
        let view = RegisterView {
            invite,
            ..RegisterView::default()
        };
        Ok(render_register(&state, &ui, StatusCode::OK, &view)?)
    }))
}

/// Create an account by using an invitation, and sign it in.
#[post("/register")]
pub async fn register_submit(
    req: HttpRequest,
    state: web::Data<AppState>,
    ui: Ui,
    form: Result<web::Form<Registration>, actix_web::Error>,
) -> HttpResponse {
    // Limit before cost (RR-0013 constraint 1): before the invitation check
    // and the password hash, and before even a malformed form is answered.
    let client = state.limiter.client_key(req.peer_addr(), req.headers());
    if let Some(throttled) = state.limiter.check_register(client) {
        return private(
            render_register_throttled(&state, &ui, form.ok().as_deref(), throttled)
                .map_err(Into::into),
        );
    }
    let result = match form {
        Ok(form) => register(&req, &state, &ui, &form).await.map_err(Into::into),
        Err(e) => Err(e),
    };
    // Every client error is a failure (RR-0013 sub-answer 1): a missing,
    // unknown, expired, revoked or used invitation, a duplicate email, or
    // invalid input. Server errors and successes don't count.
    let failed = match &result {
        Ok(resp) => resp.status().is_client_error(),
        Err(e) => e.as_response_error().status_code().is_client_error(),
    };
    if failed {
        state.limiter.record_register_failure(client);
    }
    private(result)
}

/// The register page with the 429 message, keeping the form's invitation
/// (and what was typed) when the form could be read.
fn render_register_throttled(
    state: &AppState,
    ui: &Ui,
    form: Option<&Registration>,
    throttled: Throttled,
) -> WebResult<HttpResponse> {
    let message = too_many_attempts(ui, throttled)?;
    let view = RegisterView {
        invite: form.map(|f| f.invite.as_str()).filter(|t| !t.is_empty()),
        error: Some(&message),
        display_name: form.map_or("", |f| f.display_name.as_str()),
        email: form.map_or("", |f| f.email.as_str()),
        ..RegisterView::default()
    };
    let mut resp = render_register(state, ui, StatusCode::TOO_MANY_REQUESTS, &view)?;
    resp.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from(throttled.retry_after_s),
    );
    Ok(resp)
}

/// The steps of a registration, cheapest rejection first.
async fn register(
    req: &HttpRequest,
    state: &AppState,
    ui: &Ui,
    form: &Registration,
) -> WebResult<HttpResponse> {
    // MB-0026's rate-limit check runs in `register_submit`, before this.

    // A missing or malformed token is rejected before any DB work or Argon2.
    if !token::is_well_formed(&form.invite) {
        return invalid_invitation(state, ui);
    }
    // Length before cost (RR-0022 constraint 4): reads no stored data, so it
    // answers alike whatever the invitation's state, and a refused password
    // is never hashed. Nothing of the password is logged (constraint 5).
    if check_new_password_length(&form.password).is_err() {
        tracing::debug!("registration rejected: password length");
        let message = ui.message(Message::PasswordLength)?;
        let view = RegisterView {
            invite: Some(&form.invite),
            error: Some(&message),
            display_name: &form.display_name,
            email: &form.email,
            ..RegisterView::default()
        };
        return render_register(state, ui, StatusCode::BAD_REQUEST, &view);
    }
    // A well-formed token always pays one hash before the transaction, so
    // valid and invalid ones take the same time (RR-0012 sub-answer 7).
    let password_hash = password::hash(&form.password)?;
    let outcome = InvitationRepo::register(
        &state.pool,
        &token::hash(&form.invite),
        &form.email,
        &form.display_name,
        &password_hash,
        Utc::now(),
    )
    .await;
    let registered = match outcome {
        Ok(Some(registered)) => registered,
        Ok(None) => return invalid_invitation(state, ui),
        // Kept for invitation holders (RR-0012 sub-answer 4): a form error
        // with the invitation still in the form.
        Err(CoreError::Conflict(_)) => {
            tracing::debug!("registration rejected: email already registered");
            let taken = ui.message(Message::EmailTaken)?;
            let view = RegisterView {
                invite: Some(&form.invite),
                error: Some(&taken),
                display_name: &form.display_name,
                email: &form.email,
                ..RegisterView::default()
            };
            return render_register(state, ui, StatusCode::CONFLICT, &view);
        }
        Err(e) => return Err(e.into()),
    };
    // Audit (P7): the new account and the invitation, never the token or
    // the email.
    tracing::info!(
        user_id = %registered.user.id,
        invitation_id = %registered.invitation_id,
        "invitation used: account created"
    );
    // The language chosen before the account existed follows it
    // (RR-0023 sub-answer 3): from the cookie only, never from
    // `Accept-Language`, after the register transaction committed.
    if let Some(lang) = req.cookie(COOKIE).and_then(|c| Language::parse(c.value()))
        && let Err(e) = UserRepo::set_language(&state.pool, registered.user.id, Some(lang)).await
    {
        tracing::warn!(user_id = %registered.user.id, error = ?e, "language not saved");
    }

    Identity::login(&req.extensions(), registered.user.id.to_string())
        .map_err(|e| WebError::Internal(format!("session login failed: {e}")))?;

    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/lists"))
        .finish())
}

/// The one answer to every invalid invitation: missing, malformed, unknown,
/// expired, revoked or used (RR-0012 constraint 4).
fn invalid_invitation(state: &AppState, ui: &Ui) -> WebResult<HttpResponse> {
    let message = ui.message(Message::InvalidInvitation)?;
    let view = RegisterView {
        error: Some(&message),
        ..RegisterView::default()
    };
    render_register(state, ui, StatusCode::BAD_REQUEST, &view)
}

fn render_register(
    state: &AppState,
    ui: &Ui,
    status: StatusCode,
    view: &RegisterView<'_>,
) -> WebResult<HttpResponse> {
    let view = RegisterView {
        password_min: PASSWORD_MIN_CHARS,
        ..*view
    };
    let body = ui.render(state, "auth/register", &view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
        .insert_header(VARY)
        .body(body))
}

/// Every register response, error or not, stays out of caches and never
/// sends its URL (which may hold the token) as a referrer (RR-0012
/// sub-answer 5).
fn private(result: Result<HttpResponse, actix_web::Error>) -> HttpResponse {
    let mut resp = result.unwrap_or_else(HttpResponse::from_error);
    let headers = resp.headers_mut();
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

#[post("/logout")]
pub async fn logout(id: Identity) -> HttpResponse {
    id.logout();
    HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/"))
        .finish()
}
