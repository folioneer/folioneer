//! Holding-transaction use case.
//!
//! Single cross-context orchestrator covering every operation that mutates a `Holding`
//! through a `Transaction`: opening balance, buy, sell, correct, cancel. Injects
//! `AccountService` + `AssetService` once and shares them across all five methods.
//! Will use `ensure_cash_asset(currency)` (CSH-010 helper) once cash-tracking lands.

/// Tauri command handlers for transaction-recording operations.
mod api;
/// Use-case-owned typed errors (composite + application leaf).
mod error;
/// Cross-BC orchestrator (one struct, one method per operation).
mod orchestrator;
/// Shared helpers used by the orchestrator.
mod shared;

pub use api::*;
pub use error::{
    DividendError, DividendTask, FreeSharesError, FreeSharesTask, InterestError, InterestTask,
    ManagementFeeError, ManagementFeeTask, OpenHoldingError, OpenHoldingTask, SplitError,
    SplitTask,
};
pub use orchestrator::HoldingTransactionUseCase;
