use crate::context::account::AccountError;
use crate::context::asset::AssetError;
use serde::Serialize;
use specta::Type;

/// Use-case-specific outcomes for the asset-price fetch tasks shared by
/// `FetchAllAssetPricesError` and `FetchAccountAssetPricesError`.
///
/// Carried inside each composite as a single `Failure(#[from] FetchPriceTask)` arm.
/// `#[serde(tag = "code")]` gives every variant a `{ "code": "..." }` payload so the
/// surrounding `#[serde(untagged)]` composite emits a flat, narrowable shape on the
/// wire.
#[derive(Debug, thiserror::Error, Serialize, Type, Clone)]
#[serde(tag = "code")]
pub enum FetchPriceTask {
    /// A fetch task is already in progress (MKT-113).
    #[error("A fetch task is already running")]
    FetchAlreadyRunning,
    /// No active holdings with a derivable provider symbol found in scope (MKT-111).
    #[error("No fetchable holdings in scope")]
    NoFetchableHoldings,
    /// Catch-all for unexpected runtime failures not attributable to a specific BC.
    #[error("Unexpected error")]
    UnknownError,
}

/// Wire-facing error composite for `fetch_all_asset_prices` (MKT-113, MKT-111, MKT-122).
///
/// `#[serde(untagged)]` lets every arm surface its inner `{ "code": "..." }` payload
/// directly on the wire. Each arm carries a tagged inner type (BC enum or
/// `FetchPriceTask`) so the discriminator survives the untagging.
// The `Account` arm carries the BC-wide `AccountError`; it shares `DatabaseError` and
// `DateInFuture` codes with the `Asset` arm under `#[serde(untagged)]`. Unobservable: this
// orchestrator's account calls raise only `AccountNotFound` / `DatabaseError`, and the
// frontend maps each shared code to one key regardless of which arm produced it.
#[derive(Debug, thiserror::Error, Serialize, Type)]
#[serde(untagged)]
pub enum FetchAllAssetPricesError {
    /// Propagates asset-BC failures (e.g. `DatabaseError`) via `?`.
    #[error(transparent)]
    Asset(#[from] AssetError),
    /// Propagates account-BC failures (`AccountNotFound`, `DatabaseError`) via `?`.
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-specific failures (`FetchAlreadyRunning`, `NoFetchableHoldings`, `UnknownError`).
    #[error(transparent)]
    Failure(#[from] FetchPriceTask),
}

/// Wire-facing error composite for `fetch_account_asset_prices` (MKT-113, MKT-111, MKT-132).
///
/// See `FetchAllAssetPricesError` for the shared shape rationale.
#[derive(Debug, thiserror::Error, Serialize, Type)]
#[serde(untagged)]
pub enum FetchAccountAssetPricesError {
    /// Propagates asset-BC failures (e.g. `DatabaseError`) via `?`.
    #[error(transparent)]
    Asset(#[from] AssetError),
    /// Propagates account-BC failures (`AccountNotFound`, `DatabaseError`) via `?`.
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-specific failures (`FetchAlreadyRunning`, `NoFetchableHoldings`, `UnknownError`).
    #[error(transparent)]
    Failure(#[from] FetchPriceTask),
}
