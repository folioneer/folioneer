/// Account management API handlers.
#[cfg(feature = "app")]
mod api;
/// Account domain models and traits.
mod domain;
/// Flat BC error enum (`AccountError`).
mod error;
mod profile_scope;
mod profile_service;
/// Account repository implementations.
mod repository;
/// Account business logic service.
mod service;

#[cfg(feature = "app")]
pub use api::*;
pub use domain::*;
pub use error::AccountError;
pub use profile_scope::{spawn_within_profile, within_profile};
pub use profile_service::ProfileService;
pub use repository::*;
pub use service::*;
