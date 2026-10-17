//! Holding-transaction use case.
//!
//! Single cross-context orchestrator covering every operation that mutates a `Holding`
//! through a `Transaction`: opening balance, buy, sell, correct, cancel. Injects
//! `AccountService` + `AssetService` once and shares them across all five methods.
//! Will use `ensure_cash_asset(currency)` (CSH-010 helper) once cash-tracking lands.

/// Tauri command handlers for transaction-recording operations.
#[cfg(feature = "app")]
mod api;
/// Use-case-owned typed errors (composite + application leaf).
mod error;
/// Listing by what a person named (AGT-070).
mod named_listing;
/// Recording by what a person named (CLI-011, AGT-044).
mod named_recording;
/// Cross-BC orchestrator (one struct, one method per operation).
mod orchestrator;
/// Shared helpers used by the orchestrator.
mod shared;

#[cfg(feature = "app")]
pub use api::*;
pub use error::{
    DividendError, DividendTask, FreeSharesError, FreeSharesTask, InterestError, InterestTask,
    ManagementFeeError, ManagementFeeTask, NameLookupError, OpenHoldingError, OpenHoldingTask,
    SplitError, SplitTask, TransactionDraftError, TransactionDraftTask,
};
pub use named_listing::{
    ListedTransaction, ListedTransactions, PriceRead, RecordedBy, TransactionsAsked,
    TRANSACTIONS_PER_PAGE,
};
pub use named_recording::{
    code_of, decimal, decimal_to_micro, AsShown, CashAmount, Change, ChangeAsked, Correction,
    CorrectionFields, DividendAmount, FreeShares, PriceAt, Recorded, RecordedPrice, Recording,
    Refusal, SessionRecordings, SessionRemoval, StockSplit, Target, Trade, TradeAmount,
    TypedTransaction,
};
pub use orchestrator::{
    DraftKind, HoldingTransactionUseCase, NamedTarget, OpeningBalanceDraft, OpeningBalancePreview,
    SplitSize, StockSplitDraft, StockSplitPreview, TransactionDraft, TransactionPreview,
};
