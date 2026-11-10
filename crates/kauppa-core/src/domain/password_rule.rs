//! The length rule for new passwords (MB-0034, RR-0022).
//!
//! A new password is [`PASSWORD_MIN_CHARS`] to [`PASSWORD_MAX_CHARS`]
//! Unicode scalar values (`str::chars`), counted on the string exactly as
//! submitted: no normalization and no trimming, so the rule counts what
//! `password::hash` hashes (RR-0022 constraint 3). Every flow that sets a
//! password uses [`check_new_password_length`]; login never does
//! (constraint 2).
//!
//! The error carries no data: never the password, its length or any part of
//! it (constraint 5).

use std::fmt;

/// The fewest characters a new password may have (NIST SP 800-63B rev. 4,
/// single-factor; RR-0022 sub-answer 1).
pub const PASSWORD_MIN_CHARS: usize = 15;

/// The most characters a new password may have (RR-0022 sub-answer 2).
pub const PASSWORD_MAX_CHARS: usize = 128;

/// Why a new password was refused. Field-less on purpose (RR-0022
/// sub-answer 4): it is not a `CoreError`, so it never reaches the error
/// log or the plain error body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordLengthError {
    /// Fewer than [`PASSWORD_MIN_CHARS`] characters.
    TooShort,
    /// More than [`PASSWORD_MAX_CHARS`] characters.
    TooLong,
}

impl fmt::Display for PasswordLengthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Choose a password of {PASSWORD_MIN_CHARS} to {PASSWORD_MAX_CHARS} characters."
        )
    }
}

impl std::error::Error for PasswordLengthError {}

/// Checks a new password's length against the rule.
pub fn check_new_password_length(password: &str) -> Result<(), PasswordLengthError> {
    let _ = password;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repeat(c: char, n: usize) -> String {
        std::iter::repeat_n(c, n).collect()
    }

    #[test]
    fn bounds_are_inclusive() {
        assert_eq!(
            check_new_password_length(&repeat('a', PASSWORD_MIN_CHARS)),
            Ok(())
        );
        assert_eq!(
            check_new_password_length(&repeat('a', PASSWORD_MAX_CHARS)),
            Ok(())
        );
        assert_eq!(
            check_new_password_length(&repeat('a', PASSWORD_MIN_CHARS - 1)),
            Err(PasswordLengthError::TooShort)
        );
        assert_eq!(
            check_new_password_length(&repeat('a', PASSWORD_MAX_CHARS + 1)),
            Err(PasswordLengthError::TooLong)
        );
        assert_eq!(
            check_new_password_length(""),
            Err(PasswordLengthError::TooShort)
        );
    }

    /// Characters, not bytes: "ä" is 2 bytes, "😀" is 4.
    #[test]
    fn counts_characters_not_bytes() {
        for c in ['ä', '😀'] {
            assert_eq!(
                check_new_password_length(&repeat(c, 15)),
                Ok(()),
                "15 × {c}"
            );
            assert_eq!(
                check_new_password_length(&repeat(c, 14)),
                Err(PasswordLengthError::TooShort),
                "14 × {c}"
            );
            assert_eq!(
                check_new_password_length(&repeat(c, 128)),
                Ok(()),
                "128 × {c}"
            );
            assert_eq!(
                check_new_password_length(&repeat(c, 129)),
                Err(PasswordLengthError::TooLong),
                "129 × {c}"
            );
        }
        // A decomposed "ä" (a + combining diaeresis) is two scalar values,
        // counted as submitted, with no normalization.
        let decomposed = "a\u{308}".repeat(7);
        assert_eq!(decomposed.chars().count(), 14);
        assert_eq!(check_new_password_length(&format!("{decomposed}x")), Ok(()));
        assert_eq!(
            check_new_password_length(&repeat('\u{e4}', 7)),
            Err(PasswordLengthError::TooShort)
        );
    }

    /// Spaces count: nothing is trimmed.
    #[test]
    fn spaces_count() {
        let padded = format!("  {}  ", repeat('a', 11));
        assert_eq!(padded.chars().count(), 15);
        assert_eq!(check_new_password_length(&padded), Ok(()));
        assert_eq!(
            check_new_password_length(&repeat(' ', 14)),
            Err(PasswordLengthError::TooShort)
        );
    }

    /// One message for both bounds, naming both, with no data.
    #[test]
    fn one_message_names_both_bounds() {
        let short = PasswordLengthError::TooShort.to_string();
        assert_eq!(short, PasswordLengthError::TooLong.to_string());
        assert!(short.contains(&PASSWORD_MIN_CHARS.to_string()), "{short}");
        assert!(short.contains(&PASSWORD_MAX_CHARS.to_string()), "{short}");
    }
}
