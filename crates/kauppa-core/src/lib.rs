//! Core domain, persistence, and services for `kauppa-rs`.
//!
//! Keep this crate HTTP- and framework-agnostic: no `actix_web`, no `handlebars`.
//! Web adapters live in `kauppa-web`.

pub mod domain;
pub mod error;
pub mod password;
pub mod repo;

pub use error::{CoreError, CoreResult};
