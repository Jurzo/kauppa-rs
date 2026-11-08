use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("not found")]
    NotFound,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("unauthorized")]
    Unauthorized,

    /// Signed in and allowed to see the resource, but not to do this to it
    /// (e.g. a `view` grantee adding an item to a shared list, MB-0008).
    #[error("forbidden")]
    Forbidden,

    #[error("database error")]
    Database(#[from] sqlx::Error),

    #[error("password hashing error")]
    PasswordHash(String),

    /// The OS random number generator failed (invitation tokens).
    #[error("random number generator error")]
    Random(String),
}
