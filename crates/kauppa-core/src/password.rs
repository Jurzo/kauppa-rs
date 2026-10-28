//! Password hashing using Argon2id with per-password salts.
//!
//! All auth flows must go through [`hash`] and [`verify`] — never store raw
//! passwords and never roll your own KDF. Changing the algorithm or parameters
//! here is a security-tier change (see `.agentic/mentorship/security.md`).

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

use crate::error::{CoreError, CoreResult};

pub fn hash(password: &str) -> CoreResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| CoreError::PasswordHash(e.to_string()))
}

pub fn verify(password: &str, stored_hash: &str) -> CoreResult<bool> {
    let parsed =
        PasswordHash::new(stored_hash).map_err(|e| CoreError::PasswordHash(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_matches() {
        let h = hash("correct horse battery staple").unwrap();
        assert!(verify("correct horse battery staple", &h).unwrap());
        assert!(!verify("wrong password", &h).unwrap());
    }
}
