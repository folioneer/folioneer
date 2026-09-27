//! Account Details use case: cross-context read orchestrating account + asset data (B18).

#[cfg(feature = "app")]
mod api;
mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use orchestrator::*;
