//! Account Summary use case: cross-context read enriching accounts with per-account
//! global value (CSH-094 algorithm reused at the list level for ACC-021).

mod api;
mod orchestrator;

pub use api::*;
pub use orchestrator::*;
