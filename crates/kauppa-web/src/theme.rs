//! The colour theme a visitor chose (MB-0055, RR-0035 sub-answer 1): System
//! (follow the OS), Light or Dark, kept in the `kauppa_theme` cookie. Only
//! Light and Dark are stored; System is the cookie's absence.

use actix_web::cookie::{Cookie, SameSite, time::Duration};

/// The theme cookie's name.
pub const COOKIE: &str = "kauppa_theme";

/// The cookie's lifetime: one year, as `kauppa_lang`'s.
const COOKIE_MAX_AGE_SECS: i64 = 31_536_000;

/// A theme choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Follow the OS setting.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    /// The choice a form value names: exactly `system`, `light` or `dark`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    /// The choice a `kauppa_theme` cookie value holds: Light or Dark, and
    /// System for no cookie or any other value.
    pub fn from_cookie(raw: Option<&str>) -> Self {
        match raw {
            Some("light") => Self::Light,
            Some("dark") => Self::Dark,
            _ => Self::System,
        }
    }

    /// The value templates see as `request_theme`, and the cookie holds.
    pub fn tag(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

/// The `kauppa_theme` cookie for `theme`: set for Light or Dark, removed
/// (`Max-Age=0`) for System. Attributes as `kauppa_lang`'s; `HttpOnly`,
/// since only the server reads it.
pub fn cookie(theme: Theme, secure: bool) -> Cookie<'static> {
    let (value, max_age) = match theme {
        Theme::System => ("", 0),
        Theme::Light | Theme::Dark => (theme.tag(), COOKIE_MAX_AGE_SECS),
    };
    Cookie::build(COOKIE, value)
        .path("/")
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(secure)
        .max_age(Duration::seconds(max_age))
        .finish()
}
