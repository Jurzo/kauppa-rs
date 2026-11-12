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

/// The development session secret, also in `.env.example`. Anyone can
/// forge a session signed with it, so it only serves loopback addresses
/// over plain HTTP (MB-0042, RR-0026).
pub const DEV_SESSION_SECRET: &str =
    "dev-only-dev-only-dev-only-dev-only-dev-only-dev-only-dev-only-01";

/// Refuses the development session secret unless the app binds to a
/// loopback address with `KAUPPA_COOKIE_SECURE=false` (RR-0026 sub-answer
/// 4). The error never contains a secret.
pub fn check_session_secret(secret: &str, bind: &str, cookie_secure: bool) -> anyhow::Result<()> {
    if secret != DEV_SESSION_SECRET || (is_loopback_bind(bind) && !cookie_secure) {
        return Ok(());
    }
    anyhow::bail!(
        "KAUPPA_SESSION_SECRET is the development secret from .env.example, which only \
         serves a loopback KAUPPA_BIND with KAUPPA_COOKIE_SECURE=false; set your own \
         secret, e.g. `openssl rand -base64 64 | tr -d '\\n' | cut -c1-64`"
    )
}

/// Whether `bind` (`host:port`) names a loopback address: `localhost`,
/// `127.0.0.0/8` or `::1`.
fn is_loopback_bind(bind: &str) -> bool {
    if let Ok(addr) = bind.parse::<std::net::SocketAddr>() {
        return addr.ip().is_loopback();
    }
    bind.rsplit_once(':')
        .is_some_and(|(host, _)| host.eq_ignore_ascii_case("localhost"))
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
            // Dev default; refused outside local development (RR-0026).
            .set_default("session_secret", DEV_SESSION_SECRET)?
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
        check_session_secret(
            &settings.session_secret,
            &settings.bind,
            settings.cookie_secure,
        )?;

        Ok(settings)
    }

    /// How the cross-site check learns the app's origin (RR-0010).
    pub fn origin_policy(&self) -> OriginPolicy {
        OriginPolicy::new(self.public_origin.clone(), self.cookie_secure)
    }
}

#[cfg(test)]
mod tests {
    use super::{DEV_SESSION_SECRET, check_session_secret};

    /// MB-0042 P3 (RR-0026 sub-answer 4): the development secret runs only
    /// on a loopback bind with non-secure cookies; any other secret runs
    /// anywhere. The error names the variable and never the secret.
    #[test]
    fn dev_secret_refused_outside_dev() {
        for bind in [
            "127.0.0.1:8080",
            "127.1.2.3:80",
            "[::1]:8080",
            "localhost:8080",
            "LocalHost:1",
        ] {
            assert!(
                check_session_secret(DEV_SESSION_SECRET, bind, false).is_ok(),
                "{bind}: local development"
            );
        }
        for (bind, secure) in [
            ("0.0.0.0:8080", false),
            ("192.168.1.10:8080", false),
            ("[::]:8080", false),
            ("kauppa.example:8080", false),
            ("localhost.example:8080", false),
            ("127.0.0.1:8080", true),
            ("localhost:8080", true),
            ("not an address", false),
        ] {
            let err = check_session_secret(DEV_SESSION_SECRET, bind, secure)
                .err()
                .map(|e| e.to_string());
            assert!(err.is_some(), "{bind} secure={secure}: refused");
            let err = err.unwrap_or_default();
            assert!(err.contains("KAUPPA_SESSION_SECRET"), "{err}");
            assert!(!err.contains(DEV_SESSION_SECRET), "{err}");
        }
        let other = "x".repeat(64);
        for (bind, secure) in [("0.0.0.0:8080", true), ("127.0.0.1:8080", false)] {
            assert!(check_session_secret(&other, bind, secure).is_ok(), "{bind}");
        }
    }
}
