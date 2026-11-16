use std::sync::Arc;

use kauppa_core::repo::SqlitePool;

use crate::csrf::OriginPolicy;
use crate::live::LiveHub;
use crate::routes::auth::limiter::AuthLimiter;
use crate::templates::Templates;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    /// One registry per language, and the catalogs (MB-0035, RR-0023).
    pub templates: Arc<Templates>,
    /// The app's origin for the cross-site check (`csrf`, RR-0010).
    pub origin: OriginPolicy,
    /// Failed login and registration attempts (MB-0026, RR-0013).
    pub limiter: Arc<AuthLimiter>,
    /// `KAUPPA_COOKIE_SECURE`, for the `kauppa_lang` cookie's `Secure`
    /// attribute (RR-0023 sub-answer 4).
    pub cookie_secure: bool,
    /// Announces list changes to the open list pages (MB-0057, RR-0049
    /// sub-answer 4).
    pub live: Arc<LiveHub>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
