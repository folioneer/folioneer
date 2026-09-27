//! Account Summary use case: cross-context read enriching accounts with per-account
//! global value (CSH-094 algorithm reused at the list level for ACC-021).

#[cfg(feature = "app")]
mod api;
mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use orchestrator::*;
