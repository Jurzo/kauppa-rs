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

use std::fmt;

use actix_identity::Identity;
use actix_web::http::StatusCode;
use actix_web::http::header::{self, HeaderValue};
use actix_web::{HttpMessage, HttpRequest, HttpResponse, get, post, web};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use kauppa_core::repo::{InvitationRepo, UserRepo};
use kauppa_core::{CoreError, password, token};

use crate::error::{WebError, WebResult};
use crate::state::AppState;

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
/// otherwise "registration needs an invitation".
#[derive(Debug, Default, Serialize)]
struct RegisterView<'a> {
    invite: Option<&'a str>,
    error: Option<&'static str>,
    display_name: &'a str,
    email: &'a str,
}

/// The one answer to every invalid invitation: missing, malformed, unknown,
/// expired, revoked or used (RR-0012 constraint 4).
const INVALID_INVITATION: &str = "This invitation link isn't valid. Ask for a new one.";

const EMAIL_TAKEN: &str =
    "An account already uses that email. Sign in instead, or use another email.";

#[get("/login")]
pub async fn login_form(state: web::Data<AppState>) -> WebResult<HttpResponse> {
    let body = state
        .templates
        .render("auth/login", &json!({ "error": null }))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}

#[post("/login")]
pub async fn login_submit(
    req: HttpRequest,
    state: web::Data<AppState>,
    form: web::Form<Credentials>,
) -> WebResult<HttpResponse> {
    let found = state.pool.find_by_email(&form.email).await?;
    let Some((user, stored_hash)) = found else {
        return render_login_error(&state, "Invalid email or password.");
    };

    if !password::verify(&form.password, &stored_hash)? {
        return render_login_error(&state, "Invalid email or password.");
    }

    Identity::login(&req.extensions(), user.id.to_string())
        .map_err(|e| WebError::Internal(format!("session login failed: {e}")))?;

    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/lists"))
        .finish())
}

fn render_login_error(state: &AppState, msg: &str) -> WebResult<HttpResponse> {
    let body = state
        .templates
        .render("auth/login", &json!({ "error": msg }))?;
    Ok(HttpResponse::Unauthorized()
        .content_type("text/html")
        .body(body))
}

/// The register page. With `?invite=<token>` it renders the form with the
/// token in a hidden field, without checking it; without one (or with an
/// empty one) it says registration needs an invitation.
#[get("/register")]
pub async fn register_form(
    state: web::Data<AppState>,
    query: Result<web::Query<InviteQuery>, actix_web::Error>,
) -> HttpResponse {
    private(query.and_then(|query| {
        let invite = query.invite.as_deref().filter(|t| !t.is_empty());
        let view = RegisterView {
            invite,
            ..RegisterView::default()
        };
        Ok(render_register(&state, StatusCode::OK, &view)?)
    }))
}

/// Create an account by using an invitation, and sign it in.
#[post("/register")]
pub async fn register_submit(
    req: HttpRequest,
    state: web::Data<AppState>,
    form: Result<web::Form<Registration>, actix_web::Error>,
) -> HttpResponse {
    let result = match form {
        Ok(form) => register(&req, &state, &form).await.map_err(Into::into),
        Err(e) => Err(e),
    };
    private(result)
}

/// The steps of a registration, cheapest rejection first.
async fn register(
    req: &HttpRequest,
    state: &AppState,
    form: &Registration,
) -> WebResult<HttpResponse> {
    // MB-0026's rate-limit check goes here: before the invitation check and
    // the password hash.

    // A missing or malformed token is rejected before any DB work or Argon2.
    if !token::is_well_formed(&form.invite) {
        return invalid_invitation(state);
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
        Ok(None) => return invalid_invitation(state),
        // Kept for invitation holders (RR-0012 sub-answer 4): a form error
        // with the invitation still in the form.
        Err(CoreError::Conflict(_)) => {
            tracing::debug!("registration rejected: email already registered");
            let view = RegisterView {
                invite: Some(&form.invite),
                error: Some(EMAIL_TAKEN),
                display_name: &form.display_name,
                email: &form.email,
            };
            return render_register(state, StatusCode::CONFLICT, &view);
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

    Identity::login(&req.extensions(), registered.user.id.to_string())
        .map_err(|e| WebError::Internal(format!("session login failed: {e}")))?;

    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/lists"))
        .finish())
}

fn invalid_invitation(state: &AppState) -> WebResult<HttpResponse> {
    let view = RegisterView {
        error: Some(INVALID_INVITATION),
        ..RegisterView::default()
    };
    render_register(state, StatusCode::BAD_REQUEST, &view)
}

fn render_register(
    state: &AppState,
    status: StatusCode,
    view: &RegisterView<'_>,
) -> WebResult<HttpResponse> {
    let body = state.templates.render("auth/register", view)?;
    Ok(HttpResponse::build(status)
        .content_type("text/html")
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
