//! Account Details use case: cross-context read orchestrating account + asset data (ADR-003).

mod api;
mod orchestrator;

pub use api::*;
pub use orchestrator::*;
