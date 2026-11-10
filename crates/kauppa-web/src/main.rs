// `kauppa-web` is a binary crate — `pub` on internals is just `pub(crate)` in
// effect, so suppress the lint here instead of writing `pub(crate)` everywhere.
#![allow(unreachable_pub)]
// actix-web extractors (Identity, Session) hold `Rc`, so handler futures are
// intentionally `!Send`. actix runs each handler on a thread-per-worker,
// so this is correct — just not what the pedantic lint expects.
#![allow(clippy::future_not_send)]

use std::sync::Arc;

use actix_identity::IdentityMiddleware;
use actix_session::SessionMiddleware;
use actix_session::storage::CookieSessionStore;
use actix_web::cookie::Key;
use actix_web::{App, HttpServer, web};
use tracing_actix_web::TracingLogger;

use kauppa_core::repo::SqlitePool;

mod admin;
mod config;
mod csp;
mod csrf;
mod error;
mod i18n;
mod routes;
mod state;
mod telemetry;
mod templates;

use crate::routes::auth::limiter::{AuthLimiter, SystemClock};
use crate::state::AppState;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    // .env is best-effort — production should rely on real env vars.
    let _ = dotenvy::dotenv();

    // Host subcommands (`kauppa-web admin …`, MB-0023, RR-0012), parsed
    // before anything else: unknown arguments exit non-zero and never start
    // the server.
    let command = admin::bootstrap::parse(std::env::args().skip(1))?;

    let cfg = config::Settings::load()?;
    telemetry::init(&cfg.log_level)?;

    if command.is_none() {
        tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting kauppa-web");
    }

    let pool = SqlitePool::connect(&cfg.database_url).await?;
    pool.run_migrations().await?;

    if let Some(command) = command {
        let public_origin = cfg.public_origin.as_ref().map(|p| &p.0);
        return admin::bootstrap::run(command, &pool, public_origin, &mut std::io::stdout()).await;
    }

    let origin = cfg.origin_policy();
    match &origin {
        csrf::OriginPolicy::Fixed(public) => {
            tracing::info!(origin = %public, "cross-site check uses KAUPPA_PUBLIC_ORIGIN");
        }
        csrf::OriginPolicy::SameHost(_) if cfg.cookie_secure => tracing::warn!(
            "KAUPPA_PUBLIC_ORIGIN is unset: the cross-site check trusts each request's Host; \
             set it behind a reverse proxy that rewrites Host"
        ),
        csrf::OriginPolicy::SameHost(_) => {}
    }

    let limiter = AuthLimiter::new(Arc::new(SystemClock), cfg.trusted_proxies.clone());
    // Made now, so the first unknown-email login costs what every other
    // login costs (RR-0013 sub-answer 7).
    routes::auth::dummy_hash()?;

    let templates = Arc::new(templates::Templates::load()?);
    let state = web::Data::new(AppState {
        pool: pool.clone(),
        templates,
        origin,
        limiter: Arc::new(limiter),
        cookie_secure: cfg.cookie_secure,
    });

    let session_key = Key::from(cfg.session_secret.as_bytes());
    let bind = cfg.bind.clone();

    tracing::info!(%bind, "listening");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(TracingLogger::<telemetry::RequestSpan>::new())
            .wrap(IdentityMiddleware::default())
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), session_key.clone())
                    .cookie_name("kauppa_sid".to_owned())
                    .cookie_secure(cfg.cookie_secure)
                    .cookie_http_only(true)
                    .build(),
            )
            // Outermost, so every response carries the security headers,
            // including the session/identity middleware's and `/assets`
            // (MB-0031, RR-0019). Keep it the last `.wrap`.
            .wrap(csp::headers(cfg.cookie_secure))
            // Static files (GET and HEAD only) go first: `routes::configure`
            // puts every route behind the cross-site check, in one catch-all
            // scope (RR-0010).
            .service(routes::assets::Assets)
            .configure(routes::configure)
    })
    .bind(&bind)?
    .run()
    .await?;

    Ok(())
}
