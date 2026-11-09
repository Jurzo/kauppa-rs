use std::sync::Arc;

use kauppa_core::repo::SqlitePool;

use crate::csrf::OriginPolicy;
use crate::routes::auth::limiter::AuthLimiter;
use crate::templates::Registry;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub templates: Arc<Registry>,
    /// The app's origin for the cross-site check (`csrf`, RR-0010).
    pub origin: OriginPolicy,
    /// Failed login and registration attempts (MB-0026, RR-0013).
    pub limiter: Arc<AuthLimiter>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}
