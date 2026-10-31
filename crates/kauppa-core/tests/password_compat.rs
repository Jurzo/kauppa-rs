//! Guards the password hash format across dependency-feature changes (MB-0009 P3).
//!
//! `VECTOR` was produced by `password::hash` before MB-0009 changed any crate
//! features. If it stops verifying, existing users can no longer log in.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use kauppa_core::password;

const PASSWORD: &str = "kauppa-compat-vector";
const VECTOR: &str = "$argon2id$v=19$m=19456,t=2,p=1$nnAxR/TXktCv034nPvQhtg$QYZH14S6Cs/lJql027cp1j8ztYOpE7DncOE+vmhg3ks";
const PHC_PREFIX: &str = "$argon2id$v=19$m=19456,t=2,p=1$";

#[test]
fn existing_hash_still_verifies() {
    assert!(password::verify(PASSWORD, VECTOR).unwrap());
}

#[test]
fn wrong_password_fails_against_existing_hash() {
    assert!(!password::verify("wrong password", VECTOR).unwrap());
}

#[test]
fn new_hash_keeps_phc_prefix() {
    let fresh = password::hash(PASSWORD).unwrap();
    assert!(
        fresh.starts_with(PHC_PREFIX),
        "unexpected hash format: {fresh}"
    );
    assert_ne!(fresh, VECTOR, "salt must be random per hash");
    assert!(password::verify(PASSWORD, &fresh).unwrap());
}
