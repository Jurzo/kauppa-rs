//! The language switch (MB-0035, RR-0023 sub-answer 5): `POST /language`
//! with `lang` and the page to `return` to. Registered in
//! `routes::protected`, so it is behind the cross-site check (RR-0010).

use actix_identity::Identity;
use actix_web::http::header;
use actix_web::{HttpResponse, post, web};
use serde::Deserialize;
use uuid::Uuid;

use kauppa_core::CoreError;
use kauppa_core::domain::Language;
use kauppa_core::repo::UserRepo;

use crate::error::{WebError, WebResult};
use crate::i18n::language_cookie;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LanguageForm {
    lang: String,
    #[serde(default, rename = "return")]
    return_to: String,
}

/// `path` if it is a path on this site: it starts with one `/` (not `//`)
/// and has no `\` or control characters, which browsers may read as
/// another origin or a header break. Anything else is `/`.
pub fn same_site_path(path: &str) -> &str {
    let safe = path.starts_with('/')
        && !path.starts_with("//")
        && !path.contains('\\')
        && !path.chars().any(char::is_control);
    if safe { path } else { "/" }
}

/// Sets the language: on the account when signed in, and in the
/// `kauppa_lang` cookie either way; then 303 back to the page.
#[post("/language")]
pub async fn switch(
    state: web::Data<AppState>,
    id: Option<Identity>,
    form: web::Form<LanguageForm>,
) -> WebResult<HttpResponse> {
    let lang = Language::parse(&form.lang)
        .ok_or_else(|| WebError::Core(CoreError::Invalid("unsupported language".to_owned())))?;
    let user = id
        .and_then(|i| i.id().ok())
        .and_then(|raw| Uuid::parse_str(&raw).ok());
    if let Some(user) = user {
        UserRepo::set_language(&state.pool, user, Some(lang)).await?;
    }
    Ok(HttpResponse::SeeOther()
        .insert_header((header::LOCATION, same_site_path(&form.return_to)))
        .cookie(language_cookie(lang, state.cookie_secure))
        .finish())
}
