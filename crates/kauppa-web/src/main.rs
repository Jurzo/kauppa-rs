// `kauppa-web` is a binary crate — `pub` on internals is just `pub(crate)` in
// effect, so suppress the lint here instead of writing `pub(crate)` everywhere.
#![allow(unreachable_pub)]
// actix-web extractors (Identity, Session) hold `Rc`, so handler futures are
// intentionally `!Send`. actix runs each handler on a thread-per-worker,
// so this is correct — just not what the pedantic lint expects.
#![allow(clippy::future_not_send)]

use std::sync::Arc;

use actix_files::Files;
use actix_identity::IdentityMiddleware;
use actix_session::SessionMiddleware;
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::Key;
use actix_web::{App, HttpServer, web};
use tracing_actix_web::TracingLogger;

use kauppa_core::repo::SqlitePool;

mod config;
mod error;
mod routes;
mod state;
mod telemetry;
mod templates;

use crate::state::AppState;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    // .env is best-effort — production should rely on real env vars.
    let _ = dotenvy::dotenv();

    let cfg = config::Settings::load()?;
    telemetry::init(&cfg.log_level)?;

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting kauppa-web");

    let pool = SqlitePool::connect(&cfg.database_url).await?;
    pool.run_migrations().await?;

    let templates = Arc::new(templates::build_registry()?);
    let state = web::Data::new(AppState {
        pool: pool.clone(),
        templates,
    });

    let session_key = Key::from(cfg.session_secret.as_bytes());
    let bind = cfg.bind.clone();

    tracing::info!(%bind, "listening");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(TracingLogger::default())
            .wrap(IdentityMiddleware::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                    .cookie_name("kauppa_sid".to_owned())
                    .cookie_secure(cfg.cookie_secure)
                    .cookie_http_only(true)
                    .build(),
            )
            .service(Files::new("/assets", "./assets"))
            .configure(routes::configure)
    })
    .bind(&bind)?
    .run()
    .await?;

    Ok(())
}
