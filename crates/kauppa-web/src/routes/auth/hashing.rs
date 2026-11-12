//! Password hashing for the handlers, on actix's blocking pool (MB-0046,
//! RR-0029). Argon2id takes tens of milliseconds of CPU; on an async worker
//! it would hold up every other request that worker serves. The only place
//! in this crate that calls `password::hash` or `password::verify`
//! (`tests::hashing::handlers_never_hash_directly`). `password.rs` is
//! unchanged.

use std::sync::OnceLock;

use actix_web::web;

use kauppa_core::{CoreResult, password};

use crate::error::{WebError, WebResult};

/// The password the dummy hash is made from. Its value doesn't matter: a
/// match changes nothing, since an unknown email always gets the 401.
const DUMMY_PASSWORD: &str = "kauppa-rs dummy password for unknown emails";

static DUMMY_HASH: OnceLock<String> = OnceLock::new();

/// A fixed PHC string, made once per process with `password::hash`, that
/// login verifies against for an unknown email, so every login attempt
/// costs one Argon2 verify (RR-0013 sub-answer 7). `main.rs` makes it at
/// startup, before serving, so handlers never pay for it.
pub fn dummy_hash() -> CoreResult<&'static str> {
    if let Some(hash) = DUMMY_HASH.get() {
        return Ok(hash);
    }
    let hash = password::hash(DUMMY_PASSWORD)?;
    Ok(DUMMY_HASH.get_or_init(|| hash))
}

/// Runs `work` on the blocking pool and waits for it without holding the
/// worker. A pool failure is a 500.
pub async fn on_blocking_pool<T, F>(work: F) -> WebResult<T>
where
    F: FnOnce() -> CoreResult<T> + Send + 'static,
    T: Send + 'static,
{
    web::block(work)
        .await
        .map_err(|e| WebError::Internal(format!("blocking pool: {e}")))?
        .map_err(WebError::from)
}

/// `password::hash`, off the worker.
pub async fn hash(plain: String) -> WebResult<String> {
    on_blocking_pool(move || password::hash(&plain)).await
}

/// `password::verify`, off the worker.
pub async fn verify(plain: String, stored: String) -> WebResult<bool> {
    on_blocking_pool(move || password::verify(&plain, &stored)).await
}

/// One verification against the dummy hash, off the worker: what a login
/// with an unknown email costs, so it costs what a wrong password does
/// (RR-0013 sub-answer 7, RR-0029 constraint 2). The answer is discarded.
pub async fn verify_dummy(plain: String) -> WebResult<()> {
    on_blocking_pool(move || password::verify(&plain, dummy_hash()?).map(drop)).await
}
