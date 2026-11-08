use serde::Deserialize;

use crate::csrf::{OriginPolicy, PublicOrigin};
use crate::routes::auth::limiter::TrustedProxies;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub bind: String,
    pub database_url: String,
    pub log_level: String,
    /// Must be at least 64 bytes (actix-session requirement for the master key).
    pub session_secret: String,
    pub cookie_secure: bool,
    /// The one origin browsers use to reach the app, e.g.
    /// `https://kauppa.example` (`KAUPPA_PUBLIC_ORIGIN`, RR-0010). Validated
    /// and normalized on load. Unset (or empty): the request's own `Host`.
    #[serde(default, deserialize_with = "optional_origin")]
    pub public_origin: Option<PublicOrigin>,
    /// Reverse proxies whose `X-Forwarded-For` names the client for the
    /// auth limiter (`KAUPPA_TRUSTED_PROXIES`, MB-0026, RR-0013):
    /// comma-separated addresses or CIDR blocks. Validated on load; a
    /// malformed entry refuses to start. Unset (or empty): trust none.
    #[serde(default, deserialize_with = "trusted_proxies")]
    pub trusted_proxies: TrustedProxies,
}

/// An empty `KAUPPA_PUBLIC_ORIGIN=` means unset, as in `.env.example`.
fn optional_origin<'de, D>(deserializer: D) -> Result<Option<PublicOrigin>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    raw.filter(|s| !s.trim().is_empty())
        .map(|s| PublicOrigin::try_from(s).map_err(serde::de::Error::custom))
        .transpose()
}

fn trusted_proxies<'de, D>(deserializer: D) -> Result<TrustedProxies, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    TrustedProxies::parse(raw.as_deref().unwrap_or("")).map_err(serde::de::Error::custom)
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

    /// How the cross-site check learns the app's origin (RR-0010).
    pub fn origin_policy(&self) -> OriginPolicy {
        OriginPolicy::new(self.public_origin.clone(), self.cookie_secure)
    }
}
