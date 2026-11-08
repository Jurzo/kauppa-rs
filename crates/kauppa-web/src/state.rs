use std::sync::Arc;

use kauppa_core::repo::SqlitePool;

use crate::csrf::OriginPolicy;
use crate::templates::Registry;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub templates: Arc<Registry>,
    /// The app's origin for the cross-site check (`csrf`, RR-0010).
    pub origin: OriginPolicy,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}
