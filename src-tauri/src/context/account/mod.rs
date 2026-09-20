/// Account management API handlers.
mod api;
/// Account domain models and traits.
mod domain;
/// Flat BC error enum (`AccountError`).
mod error;
/// Account repository implementations.
mod repository;
/// Account business logic service.
mod service;

pub use api::*;
pub use domain::*;
pub use error::AccountError;
pub use repository::*;
pub use service::*;
