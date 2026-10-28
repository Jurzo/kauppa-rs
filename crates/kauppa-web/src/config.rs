use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub bind: String,
    pub database_url: String,
    pub log_level: String,
    /// Must be at least 64 bytes (actix-session requirement for the master key).
    pub session_secret: String,
    pub cookie_secure: bool,
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let builder = config::Config::builder()
            .set_default("bind", "127.0.0.1:8080")?
            .set_default("database_url", "sqlite://data/kauppa.db")?
            .set_default("log_level", "info")?
            .set_default("cookie_secure", false)?
            // Dev default; MUST be overridden via KAUPPA_SESSION_SECRET in prod.
            .set_default(
                "session_secret",
                "dev-only-dev-only-dev-only-dev-only-dev-only-dev-only-dev-only-01",
            )?
            // Env vars are `KAUPPA_<FIELD>` (single underscore). `database_url` field
            // ← `KAUPPA_DATABASE_URL`, `session_secret` ← `KAUPPA_SESSION_SECRET`, etc.
            .add_source(config::Environment::with_prefix("KAUPPA").try_parsing(true));

        let settings: Self = builder.build()?.try_deserialize()?;

        if settings.session_secret.len() < 64 {
            anyhow::bail!(
                "KAUPPA_SESSION_SECRET must be at least 64 bytes (got {})",
                settings.session_secret.len()
            );
        }

        Ok(settings)
    }
}
