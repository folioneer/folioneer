//! Account Performance use case: cross-context recompute-on-read of per-period
//! values and Simple Dietz performance metrics (PRF spec, ADR-013).

mod api;
mod orchestrator;

pub use crate::use_cases::shared::performance::{AccountPerformanceResponse, PerformancePeriod};
pub use crate::use_cases::shared::valuation::PerformanceMetric;
pub use api::*;
pub use orchestrator::AccountPerformanceUseCase;
