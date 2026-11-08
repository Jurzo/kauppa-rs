//! Invitation tokens (MB-0023, RR-0012 sub-answer 6).
//!
//! A token is 32 bytes (256 bits) from the OS RNG, written as 64 lower-case
//! hex characters. Only its hex SHA-256 is stored; the plaintext leaves this
//! crate once, as the [`InviteToken`] returned when an invitation is created,
//! and is never logged (its `Debug` is redacted).

use std::fmt;

use argon2::password_hash::rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

use crate::error::{CoreError, CoreResult};

/// Random bytes in a token.
const TOKEN_BYTES: usize = 32;

/// Characters in a token's text form.
pub const TOKEN_LEN: usize = TOKEN_BYTES * 2;

/// A plaintext invitation token. Shown to the admin once; never stored.
pub struct InviteToken(String);

impl InviteToken {
    /// A new token from the OS RNG.
    pub fn generate() -> CoreResult<Self> {
        let mut bytes = [0u8; TOKEN_BYTES];
        OsRng
            .try_fill_bytes(&mut bytes)
            .map_err(|e| CoreError::Random(e.to_string()))?;
        Ok(Self(to_hex(&bytes)))
    }

    /// The token as it goes in the link.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The stored form, [`hash`] of this token.
    #[must_use]
    pub fn hash(&self) -> String {
        hash(&self.0)
    }
}

impl fmt::Debug for InviteToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InviteToken(<redacted>)")
    }
}

/// Whether `token` has a token's shape: exactly 64 lower-case hex
/// characters. Checked before any expensive work, so garbage costs nothing.
#[must_use]
pub fn is_well_formed(token: &str) -> bool {
    token.len() == TOKEN_LEN
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The stored form of a token: its SHA-256 as 64 lower-case hex characters.
#[must_use]
pub fn hash(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_hash_known_values() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xa5, 0xff]), "000fa5ff");
        // SHA-256("abc"), FIPS 180-2 appendix B.1.
        assert_eq!(
            hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn generated_tokens_are_well_formed_and_distinct() {
        let a = InviteToken::generate().unwrap();
        let b = InviteToken::generate().unwrap();
        assert!(is_well_formed(a.as_str()), "{}", a.as_str());
        assert_ne!(a.as_str(), b.as_str());
        assert_eq!(a.hash(), hash(a.as_str()));
        assert_ne!(a.hash(), a.as_str());
        assert!(!format!("{a:?}").contains(a.as_str()), "Debug redacts");
    }

    #[test]
    fn shape_check() {
        assert!(is_well_formed(&"0a".repeat(32)));
        assert!(!is_well_formed(""));
        assert!(!is_well_formed(&"0a".repeat(31)));
        assert!(!is_well_formed(&"0a".repeat(33)));
        assert!(!is_well_formed(&"0A".repeat(32)), "upper case");
        assert!(!is_well_formed(&"0g".repeat(32)), "not hex");
        assert!(!is_well_formed(&"é".repeat(32)), "64 bytes, not hex");
    }
}
