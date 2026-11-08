//! The auth limiter (MB-0026, RR-0013): fixed windows of *failed* login
//! and registration attempts, per client and per account, consulted by the
//! login and register handlers before any database or Argon2 work.
//!
//! Each map holds at most [`MAX_KEYS`] entries. Account keys are 64-bit
//! hashes of the normalized email, so no email is held in memory, and no
//! log line here names an email, an address or a key.

use std::collections::HashMap;
use std::collections::hash_map::{Entry as MapEntry, RandomState};
use std::fmt;
use std::hash::{BuildHasher, Hash};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use actix_web::http::header::HeaderMap;

pub mod client;

#[cfg(test)]
mod tests;

pub use client::{ClientKey, Resolved, TrustedProxies};

/// A limit: this many failures per key within one fixed window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    pub failures: u32,
    pub window: Duration,
}

/// Failed `POST /auth/login` per client (RR-0013 sub-answer 1, P1).
pub const LOGIN_PER_CLIENT: Limit = Limit {
    failures: 10,
    window: Duration::from_secs(15 * 60),
};

/// Failed `POST /auth/login` per normalized email, existing or not (P2).
pub const LOGIN_PER_ACCOUNT: Limit = Limit {
    failures: 5,
    window: Duration::from_secs(15 * 60),
};

/// Failed `POST /auth/register` per client (P3).
pub const REGISTER_PER_CLIENT: Limit = Limit {
    failures: 10,
    window: Duration::from_secs(60 * 60),
};

/// Entries per map (P4).
pub const MAX_KEYS: usize = 10_000;

/// Where the limiter's time comes from; tests inject a fake one.
pub trait Clock: Send + Sync + fmt::Debug {
    fn now(&self) -> Instant;
}

/// The monotonic system clock.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// A key at its limit: try again after `retry_after`, in whole seconds,
/// rounded up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throttled {
    pub retry_after_s: u64,
}

impl Throttled {
    /// `retry_after_s` in whole minutes, rounded up, for the page.
    pub const fn minutes(self) -> u64 {
        self.retry_after_s.div_ceil(60)
    }
}

/// Which map a check or a log line is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    LoginPerClient,
    LoginPerAccount,
    RegisterPerClient,
}

impl Which {
    /// The throttle line's `limiter` field.
    pub const fn limiter(self) -> &'static str {
        match self {
            Self::LoginPerClient | Self::LoginPerAccount => "login",
            Self::RegisterPerClient => "register",
        }
    }

    /// The throttle line's `key_kind` field.
    pub const fn key_kind(self) -> &'static str {
        match self {
            Self::LoginPerClient | Self::RegisterPerClient => "client",
            Self::LoginPerAccount => "account",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    window_start: Instant,
    failures: u32,
}

/// One fixed-window map.
#[derive(Debug)]
struct Window<K> {
    which: Which,
    limit: Limit,
    state: Mutex<WindowState<K>>,
}

#[derive(Debug)]
struct WindowState<K> {
    entries: HashMap<K, Entry>,
    /// When the capacity WARN last fired: at most one per window.
    capacity_warned_at: Option<Instant>,
}

impl<K: Copy + Eq + Hash> Window<K> {
    fn new(which: Which, limit: Limit) -> Self {
        Self {
            which,
            limit,
            state: Mutex::new(WindowState {
                entries: HashMap::new(),
                capacity_warned_at: None,
            }),
        }
    }

    /// The lock, recovered if a holder panicked: each update is a single
    /// assignment, so the map is never half-written.
    fn lock(&self) -> std::sync::MutexGuard<'_, WindowState<K>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn expired(&self, entry: Entry, now: Instant) -> bool {
        now.saturating_duration_since(entry.window_start) >= self.limit.window
    }

    /// `Some` if `key` is at its limit in its current window. An expired
    /// entry counts as absent and is removed.
    fn check(&self, key: K, now: Instant) -> Option<Throttled> {
        let mut state = self.lock();
        let entry = *state.entries.get(&key)?;
        if self.expired(entry, now) {
            // Under the same lock: a concurrent failure may not restart the
            // window between the read and the removal.
            state.entries.remove(&key);
            drop(state);
            return None;
        }
        drop(state);
        if entry.failures < self.limit.failures {
            return None;
        }
        let left = (entry.window_start + self.limit.window).saturating_duration_since(now);
        let retry_after_s = left.as_secs() + u64::from(left.subsec_nanos() > 0);
        Some(Throttled {
            retry_after_s: retry_after_s.max(1),
        })
    }

    /// Counts one failure for `key`, starting a window if it has none.
    fn record(&self, key: K, now: Instant) {
        let mut state = self.lock();
        if state.entries.len() >= MAX_KEYS && !state.entries.contains_key(&key) {
            self.make_room(&mut state, now);
        }
        let fresh = Entry {
            window_start: now,
            failures: 1,
        };
        match state.entries.entry(key) {
            MapEntry::Occupied(mut occupied) => {
                let entry = occupied.get_mut();
                if self.expired(*entry, now) {
                    *entry = fresh;
                } else {
                    entry.failures = entry.failures.saturating_add(1);
                }
            }
            MapEntry::Vacant(vacant) => {
                vacant.insert(fresh);
            }
        }
    }

    /// A full map: purge every expired entry, then, if still full, evict
    /// the one with the oldest window (RR-0013 sub-answer 5).
    fn make_room(&self, state: &mut WindowState<K>, now: Instant) {
        let window = self.limit.window;
        state.entries.retain(|_, e| !self.expired(*e, now));
        if state.entries.len() < MAX_KEYS {
            return;
        }
        let oldest = state
            .entries
            .iter()
            .min_by_key(|(_, e)| e.window_start)
            .map(|(k, _)| *k);
        if let Some(oldest) = oldest {
            state.entries.remove(&oldest);
        }
        let warned_recently = state
            .capacity_warned_at
            .is_some_and(|at| now.saturating_duration_since(at) < window);
        if !warned_recently {
            state.capacity_warned_at = Some(now);
            tracing::warn!(
                limiter = self.which.limiter(),
                key_kind = self.which.key_kind(),
                "auth limiter at capacity; evicting oldest"
            );
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.lock().entries.len()
    }
}

/// The three maps, the clock, the trusted proxies and the account-key
/// hasher. One per process, in `AppState`.
pub struct AuthLimiter {
    clock: Arc<dyn Clock>,
    trusted: TrustedProxies,
    account_hasher: RandomState,
    login_client: Window<ClientKey>,
    login_account: Window<u64>,
    register_client: Window<ClientKey>,
    /// The trusted-proxy fallback WARN fires once per process.
    fallback_warned: AtomicBool,
}

impl fmt::Debug for AuthLimiter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthLimiter").finish_non_exhaustive()
    }
}

impl AuthLimiter {
    pub fn new(clock: Arc<dyn Clock>, trusted: TrustedProxies) -> Self {
        Self {
            clock,
            trusted,
            account_hasher: RandomState::new(),
            login_client: Window::new(Which::LoginPerClient, LOGIN_PER_CLIENT),
            login_account: Window::new(Which::LoginPerAccount, LOGIN_PER_ACCOUNT),
            register_client: Window::new(Which::RegisterPerClient, REGISTER_PER_CLIENT),
            fallback_warned: AtomicBool::new(false),
        }
    }

    /// The request's client key (RR-0013 sub-answer 2). Warns once per
    /// process when a trusted proxy sends no usable `X-Forwarded-For`.
    pub fn client_key(&self, peer: Option<SocketAddr>, headers: &HeaderMap) -> ClientKey {
        let resolved = self.trusted.resolve(peer, headers);
        if matches!(resolved, Resolved::ProxyFallback(_))
            && !self.fallback_warned.swap(true, Ordering::Relaxed)
        {
            tracing::warn!("forwarded-for missing from a trusted proxy");
        }
        resolved.key()
    }

    /// The account key: a hash of the trimmed, ASCII-lowercased email.
    pub fn account_key(&self, email: &str) -> u64 {
        self.account_hasher
            .hash_one(email.trim().to_ascii_lowercase())
    }

    /// Login's checks, client then account. Logs the throttle line.
    pub fn check_login(&self, client: ClientKey, account: u64) -> Option<Throttled> {
        let now = self.clock.now();
        throttled(Which::LoginPerClient, self.login_client.check(client, now)).or_else(|| {
            throttled(
                Which::LoginPerAccount,
                self.login_account.check(account, now),
            )
        })
    }

    /// A failed login counts against both the client and the account.
    pub fn record_login_failure(&self, client: ClientKey, account: u64) {
        let now = self.clock.now();
        self.login_client.record(client, now);
        self.login_account.record(account, now);
    }

    /// Register's check. Logs the throttle line.
    pub fn check_register(&self, client: ClientKey) -> Option<Throttled> {
        let now = self.clock.now();
        throttled(
            Which::RegisterPerClient,
            self.register_client.check(client, now),
        )
    }

    pub fn record_register_failure(&self, client: ClientKey) {
        self.register_client.record(client, self.clock.now());
    }

    /// Entries per map: login client, login account, register client.
    #[cfg(test)]
    pub fn lens(&self) -> [usize; 3] {
        [
            self.login_client.len(),
            self.login_account.len(),
            self.register_client.len(),
        ]
    }
}

/// The throttle line (RR-0013 sub-answer 6): the limiter and the key kind,
/// never the email, the password or the address.
fn throttled(which: Which, outcome: Option<Throttled>) -> Option<Throttled> {
    if let Some(t) = outcome {
        tracing::warn!(
            limiter = which.limiter(),
            key_kind = which.key_kind(),
            retry_after_s = t.retry_after_s,
            "auth attempt throttled"
        );
    }
    outcome
}

/// A clock tests move by hand (RR-0013 constraint 7: never `sleep`).
#[cfg(test)]
#[derive(Debug)]
pub struct FakeClock(Mutex<Instant>);

#[cfg(test)]
impl Default for FakeClock {
    fn default() -> Self {
        Self(Mutex::new(Instant::now()))
    }
}

#[cfg(test)]
impl FakeClock {
    pub fn advance(&self, by: Duration) {
        let mut now = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        *now += by;
    }
}

#[cfg(test)]
impl Clock for FakeClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
