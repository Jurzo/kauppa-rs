//! Authentication endpoints.
//!
//! Password hashing goes through `kauppa_core::password` — never hash in a
//! handler. Session lifetime is controlled by `actix-identity` on top of
//! `actix-session`; `Identity::login` writes the session cookie.

use actix_identity::Identity;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, get, http::header, post, web};
use serde::Deserialize;
use serde_json::json;

use kauppa_core::password;
use kauppa_core::repo::UserRepo;

use crate::error::{WebError, WebResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct Registration {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

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

#[get("/register")]
pub async fn register_form(state: web::Data<AppState>) -> WebResult<HttpResponse> {
    let body = state
        .templates
        .render("auth/register", &json!({ "error": null }))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(body))
}

#[post("/register")]
pub async fn register_submit(
    req: HttpRequest,
    state: web::Data<AppState>,
    form: web::Form<Registration>,
) -> WebResult<HttpResponse> {
    let hashed = password::hash(&form.password)?;
    let user = state
        .pool
        .create(&form.email, &form.display_name, &hashed)
        .await?;

    Identity::login(&req.extensions(), user.id.to_string())
        .map_err(|e| WebError::Internal(format!("session login failed: {e}")))?;

    Ok(HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/lists"))
        .finish())
}

#[post("/logout")]
pub async fn logout(id: Identity) -> HttpResponse {
    id.logout();
    HttpResponse::SeeOther()
        .append_header((header::LOCATION, "/"))
        .finish()
}
