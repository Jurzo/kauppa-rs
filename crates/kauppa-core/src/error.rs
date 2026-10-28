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

    #[error("database error")]
    Database(#[from] sqlx::Error),

    #[error("password hashing error")]
    PasswordHash(String),
}
