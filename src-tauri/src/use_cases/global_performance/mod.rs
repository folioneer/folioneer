//! Global Performance use case: portfolio-wide recompute-on-read performance
//! (GPF spec) — all accounts, or one asset's positions across all accounts,
//! aggregated in the reference currency; single-account scopes reuse the shared
//! performance series engine.

mod api;
mod orchestrator;

pub use api::*;
pub use orchestrator::GlobalPerformanceUseCase;
