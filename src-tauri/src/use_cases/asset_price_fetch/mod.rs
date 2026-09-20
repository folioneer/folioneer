//! Auto-fetch use case: retrieves prices from Yahoo for all active, derivable
//! holdings on launch (MKT-122), on global refresh (MKT-130), and on
//! per-account refresh (MKT-132).

/// Tauri command handlers for fetch tasks.
pub mod api;
/// Background task dispatcher — runs per-asset HTTP fetch + upsert.
pub mod dispatcher;
/// Use-case-specific failure codes shared by both composites + the two composites.
pub mod error;
/// In-flight fetch guard (MKT-113) — RAII lease pattern.
pub mod guard;
/// Price Movement baseline capture (PMV-020/021) — the stateful half of the
/// Price Movement report.
pub mod movement_capture;
/// Orchestrator with `fetch_all` and `fetch_for_account` methods.
pub mod orchestrator;
#[cfg(test)]
mod serde_check;
/// Which action started an all-accounts fetch (PMV-010/015).
pub mod trigger;

pub use api::*;
pub use error::{FetchAccountAssetPricesError, FetchAllAssetPricesError, FetchPriceTask};
pub use guard::FetchGuard;
pub use orchestrator::AssetPriceFetchUseCase;
pub use trigger::FetchTrigger;
