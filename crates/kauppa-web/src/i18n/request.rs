//! The language of a request (RR-0023 sub-answer 3): the signed-in user's
//! `users.language`, then the `kauppa_lang` cookie, then `Accept-Language`,
//! then English.

use std::future::Future;
use std::pin::Pin;

use actix_identity::Identity;
use actix_web::cookie::{Cookie, SameSite, time::Duration};
use actix_web::dev::Payload;
use actix_web::http::header::{self, HeaderName};
use actix_web::{FromRequest, HttpRequest, web};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use kauppa_core::domain::Language;
use kauppa_core::repo::UserRepo;
use kauppa_core::{Conflict, Invalid};

use super::messages::{self, Message, Text};
use crate::error::{WebError, WebResult};
use crate::state::AppState;

/// The language cookie's name (RR-0023 sub-answer 4).
pub const COOKIE: &str = "kauppa_lang";

/// The cookie's lifetime: one year.
const COOKIE_MAX_AGE_SECS: i64 = 31_536_000;

/// HTML responses vary by the inputs that choose their language.
pub const VARY: (HeaderName, &str) = (header::VARY, "Cookie, Accept-Language");

/// The root-context key the layout reads for the language switcher's
/// `return` field.
const REQUEST_PATH: &str = "request_path";

/// The `kauppa_lang` cookie choosing `lang`.
pub fn language_cookie(lang: Language, secure: bool) -> Cookie<'static> {
    Cookie::build(COOKIE, lang.tag())
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(secure)
        .max_age(Duration::seconds(COOKIE_MAX_AGE_SECS))
        .finish()
}

/// The supported language `Accept-Language` prefers: the highest `q`
/// among entries whose primary subtag is supported (the first on a tie);
/// `q=0`, malformed `q` values and `*` choose nothing.
pub fn from_accept_language(value: &str) -> Option<Language> {
    let mut best: Option<(Language, f32)> = None;
    for entry in value.split(',') {
        let mut parts = entry.split(';');
        let tag = parts.next().unwrap_or_default().trim();
        let primary = tag
            .split('-')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let Some(lang) = Language::parse(&primary) else {
            continue;
        };
        let mut q = 1.0_f32;
        for param in parts {
            if let Some(v) = param.trim().strip_prefix("q=") {
                match v.trim().parse::<f32>() {
                    Ok(v) if (0.0..=1.0).contains(&v) => q = v,
                    _ => q = 0.0,
                }
            }
        }
        if q > 0.0 && best.is_none_or(|(_, b)| q > b) {
            best = Some((lang, q));
        }
    }
    best.map(|(lang, _)| lang)
}

/// What a handler needs to answer in the request's language: the language,
/// and the path the switcher returns to.
#[derive(Debug, Clone)]
pub struct Ui {
    pub lang: Language,
    path: String,
    state: web::Data<AppState>,
}

impl Ui {
    /// The app state, for helpers that would otherwise take it beside the
    /// `Ui` (clippy's argument limit).
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Renders `name` with `view` in this request's language. The root
    /// context also gets the request path, which the layout's switcher
    /// returns to.
    pub fn render<T: Serialize>(
        &self,
        state: &AppState,
        name: &str,
        view: &T,
    ) -> WebResult<String> {
        let mut data = serde_json::to_value(view)
            .map_err(|e| WebError::Internal(format!("view of {name}: {e}")))?;
        if let Value::Object(map) = &mut data {
            map.insert(REQUEST_PATH.to_owned(), Value::String(self.path.clone()));
        }
        Ok(state.templates.get(self.lang).render(name, &data)?)
    }

    /// Rust-built text in this request's language (unescaped: a view
    /// field rendered with `{{…}}` escapes it).
    pub fn text(&self, key: &str, args: &[(&str, String)]) -> WebResult<String> {
        self.state
            .templates
            .i18n()
            .text(self.lang, key, args)
            .map_err(|e| WebError::Internal(e.to_string()))
    }

    /// A key and arguments from [`messages`](super::messages), as text.
    pub fn say(&self, (key, args): Text) -> WebResult<String> {
        self.text(&key, &args)
    }

    /// Why input was rejected, as text.
    pub fn invalid(&self, reason: &Invalid) -> WebResult<String> {
        self.say(messages::invalid(reason))
    }

    /// Why a write conflicted, as text.
    pub fn conflict(&self, reason: &Conflict) -> WebResult<String> {
        self.say(messages::conflict(reason))
    }

    /// A web-only message, as text.
    pub fn message(&self, message: Message) -> WebResult<String> {
        self.say(messages::message(message))
    }

    /// `at` in this language's date format.
    pub fn date(&self, state: &AppState, at: DateTime<Utc>) -> WebResult<String> {
        state
            .templates
            .i18n()
            .date(self.lang, at)
            .map_err(|e| WebError::Internal(e.to_string()))
    }

    /// `at` in this language's date-and-time format.
    pub fn datetime(&self, state: &AppState, at: DateTime<Utc>) -> WebResult<String> {
        state
            .templates
            .i18n()
            .datetime(self.lang, at)
            .map_err(|e| WebError::Internal(e.to_string()))
    }
}

impl FromRequest for Ui {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let user = Identity::from_request(req, payload)
            .into_inner()
            .ok()
            .and_then(|id| id.id().ok())
            .and_then(|raw| Uuid::parse_str(&raw).ok());
        let state = req.app_data::<web::Data<AppState>>().cloned();
        let cookie = req.cookie(COOKIE).and_then(|c| Language::parse(c.value()));
        let header = req
            .headers()
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
            .and_then(from_accept_language);
        let path = req
            .uri()
            .path_and_query()
            .map_or_else(|| "/".to_owned(), |p| p.as_str().to_owned());
        Box::pin(async move {
            let state = state.ok_or_else(|| WebError::Internal("no app state".to_owned()))?;
            let stored = match user {
                Some(user) => UserRepo::language(&state.pool, user)
                    .await
                    .map_err(WebError::from)?,
                None => None,
            };
            let lang = stored.or(cookie).or(header).unwrap_or(Language::DEFAULT);
            Ok(Self { lang, path, state })
        })
    }
}
