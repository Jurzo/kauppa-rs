use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init(default_level: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_env("KAUPPA_LOG")
        .or_else(|_| EnvFilter::try_new(default_level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    // JSON in release, pretty in debug — keeps dev readable, prod structured.
    #[cfg(debug_assertions)]
    let layer = fmt::layer().with_target(true).with_level(true).compact();

    #[cfg(not(debug_assertions))]
    let layer = fmt::layer().json().flatten_event(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(layer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("failed to install tracing subscriber: {e}"))?;

    Ok(())
}
