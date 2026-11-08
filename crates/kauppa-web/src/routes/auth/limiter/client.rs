//! Which client a request comes from, for the auth limiter (RR-0013
//! sub-answer 2, constraint 2).
//!
//! The client is the TCP peer, or, only when the peer is one of
//! `KAUPPA_TRUSTED_PROXIES`, the right-most untrusted `X-Forwarded-For`
//! entry. Nothing here reads `Forwarded`, `X-Real-IP` or
//! `realip_remote_addr()`; the cross-site check (RR-0010) reads no forwarded
//! header at all.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;

use actix_web::http::header::HeaderMap;

/// The only forwarded header the limiter reads.
const X_FORWARDED_FOR: &str = "x-forwarded-for";

/// One `KAUPPA_TRUSTED_PROXIES` entry: an address, or a CIDR block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Net {
    V4 { addr: u32, prefix: u8 },
    V6 { addr: u128, prefix: u8 },
}

impl Net {
    fn contains(self, ip: IpAddr) -> bool {
        match (self, canonical(ip)) {
            (Self::V4 { addr, prefix }, IpAddr::V4(ip)) => mask_v4(u32::from(ip), prefix) == addr,
            (Self::V6 { addr, prefix }, IpAddr::V6(ip)) => mask_v6(u128::from(ip), prefix) == addr,
            _ => false,
        }
    }
}

impl FromStr for Net {
    type Err = String;

    fn from_str(entry: &str) -> Result<Self, Self::Err> {
        let invalid = || format!("`{entry}` is not an IP address or CIDR block");
        let (addr, prefix) = match entry.split_once('/') {
            Some((addr, prefix)) => (addr, Some(prefix)),
            None => (entry, None),
        };
        let addr = canonical(IpAddr::from_str(addr).map_err(|_| invalid())?);
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            // Digits only: `u8::from_str` would also take a leading `+`.
            Some(p) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
                p.parse::<u8>().map_err(|_| invalid())?
            }
            Some(_) => return Err(invalid()),
            None => max,
        };
        if prefix > max {
            return Err(invalid());
        }
        Ok(match addr {
            IpAddr::V4(ip) => Self::V4 {
                addr: mask_v4(u32::from(ip), prefix),
                prefix,
            },
            IpAddr::V6(ip) => Self::V6 {
                addr: mask_v6(u128::from(ip), prefix),
                prefix,
            },
        })
    }
}

fn mask_v4(addr: u32, prefix: u8) -> u32 {
    u32::MAX
        .checked_shl(32 - u32::from(prefix))
        .map_or(0, |mask| addr & mask)
}

fn mask_v6(addr: u128, prefix: u8) -> u128 {
    u128::MAX
        .checked_shl(128 - u32::from(prefix))
        .map_or(0, |mask| addr & mask)
}

/// An IPv4-mapped IPv6 address as the IPv4 address it is.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

/// `KAUPPA_TRUSTED_PROXIES`: comma-separated addresses or CIDR blocks.
/// Empty trusts no peer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedProxies(Vec<Net>);

impl TrustedProxies {
    /// Parses the setting; any malformed entry is an error, so the app
    /// refuses to start (RR-0013 sub-answer 2). Blank means none.
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        raw.split(',')
            .map(|entry| entry.trim().parse::<Net>())
            .collect::<Result<_, _>>()
            .map(Self)
            .map_err(|e| format!("KAUPPA_TRUSTED_PROXIES: {e}"))
    }

    #[cfg(test)]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn contains(&self, ip: IpAddr) -> bool {
        self.0.iter().any(|net| net.contains(ip))
    }

    /// The request's client (RR-0013 sub-answer 2, steps 1–4).
    pub fn resolve(&self, peer: Option<SocketAddr>, headers: &HeaderMap) -> Resolved {
        let Some(peer) = peer.map(|p| canonical(p.ip())) else {
            return Resolved::Client(ClientKey::UnknownPeer);
        };
        if !self.contains(peer) {
            return Resolved::Client(ClientKey::from(peer));
        }
        self.right_most_untrusted(headers).map_or_else(
            || Resolved::ProxyFallback(ClientKey::from(peer)),
            |client| Resolved::Client(ClientKey::from(client)),
        )
    }

    /// Every `X-Forwarded-For` line joined in order, walked from the right
    /// past trusted entries. `None` if the header is missing, empty or
    /// unparsable, or every entry is trusted.
    fn right_most_untrusted(&self, headers: &HeaderMap) -> Option<IpAddr> {
        let mut lines = Vec::new();
        for value in headers.get_all(X_FORWARDED_FOR) {
            lines.push(value.to_str().ok()?);
        }
        let joined = lines.join(",");
        for entry in joined.rsplit(',') {
            let ip = canonical(IpAddr::from_str(entry.trim()).ok()?);
            if !self.contains(ip) {
                return Some(ip);
            }
        }
        None
    }
}

/// The outcome of [`TrustedProxies::resolve`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolved {
    /// The client, from the peer or a trusted proxy's header.
    Client(ClientKey),
    /// A trusted proxy sent no usable `X-Forwarded-For`: the proxy's own
    /// address, the shared bucket (step 3).
    ProxyFallback(ClientKey),
}

impl Resolved {
    pub const fn key(self) -> ClientKey {
        match self {
            Self::Client(key) | Self::ProxyFallback(key) => key,
        }
    }
}

/// A per-client limiter key: an IPv4 address, an IPv6 /64, or the one
/// bucket for requests with no peer address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientKey {
    V4(Ipv4Addr),
    /// The upper 64 bits of the address.
    V6Prefix64(u64),
    UnknownPeer,
}

impl From<IpAddr> for ClientKey {
    fn from(ip: IpAddr) -> Self {
        match canonical(ip) {
            IpAddr::V4(v4) => Self::V4(v4),
            IpAddr::V6(v6) => {
                // The upper half of a `u128` always fits in a `u64`.
                Self::V6Prefix64(u64::try_from(u128::from(v6) >> 64).unwrap_or(u64::MAX))
            }
        }
    }
}

impl ClientKey {
    /// The key truncated to IPv4 /24 or IPv6 /48 for the request span
    /// (RR-0013 sub-answer 6); empty for the unknown-peer bucket.
    pub const fn truncated(self) -> Truncated {
        Truncated(self)
    }
}

/// [`ClientKey::truncated`]'s display form, e.g. `203.0.113.0/24`.
#[derive(Debug, Clone, Copy)]
pub struct Truncated(ClientKey);

impl fmt::Display for Truncated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            ClientKey::V4(v4) => {
                let net = Ipv4Addr::from(mask_v4(u32::from(v4), 24));
                write!(f, "{net}/24")
            }
            ClientKey::V6Prefix64(upper) => {
                let net = Ipv6Addr::from(mask_v6(u128::from(upper) << 64, 48));
                write!(f, "{net}/48")
            }
            ClientKey::UnknownPeer => Ok(()),
        }
    }
}
