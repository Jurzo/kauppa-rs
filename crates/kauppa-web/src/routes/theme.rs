//! The theme switch (MB-0055, RR-0035 sub-answer 2): `POST /theme` with
//! `theme` and the page to `return` to. Registered in `routes::protected`,
//! so it is behind the cross-site check (RR-0010).

use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, post, web};
use kauppa_core::{CoreError, Invalid};
use serde::Deserialize;

use crate::error::{WebError, WebResult};
use crate::routes::language::same_site_path;
use crate::routes::request::is_hx;
use crate::state::AppState;
use crate::theme::{Theme, cookie};

#[derive(Debug, Deserialize)]
pub struct ThemeForm {
    #[serde(default)]
    theme: String,
    #[serde(default, rename = "return")]
    return_to: String,
}

/// Stores the choice in the `kauppa_theme` cookie (removes it for System).
/// The switch posts with htmx on change and needs nothing back: 204. The
/// no-JS Save button posts a plain form: 303 back to the page.
#[post("/theme")]
pub async fn switch(
    req: HttpRequest,
    state: web::Data<AppState>,
    form: web::Form<ThemeForm>,
) -> WebResult<HttpResponse> {
    let theme = Theme::parse(&form.theme)
        .ok_or_else(|| WebError::Core(CoreError::Invalid(Invalid::Theme)))?;
    let cookie = cookie(theme, state.cookie_secure);
    if is_hx(&req) {
        return Ok(HttpResponse::NoContent().cookie(cookie).finish());
    }
    Ok(HttpResponse::SeeOther()
        .insert_header((header::LOCATION, same_site_path(&form.return_to)))
        .cookie(cookie)
        .finish())
}

#[cfg(test)]
mod tests;
