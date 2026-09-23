//! Application use cases layer.
//!
//! Cross-cutting application use cases that orchestrate multiple bounded
//! contexts or platform capabilities.

/// Account Creation: seeds the per-currency Cash Asset + the account's 0-balance Cash Holding at creation (ACC-025, CSH-010/012).
pub mod account_creation;
/// Account Deletion: pre-deletion summary of holdings and transactions (ACC-020).
pub mod account_deletion;
/// Account Details: cross-context read of holdings + asset metadata (ACD feature),
/// live or reconstructed as of a past date.
pub mod account_details;
/// Account Performance: recompute-on-read per-period values and Simple Dietz metrics (PRF spec, ADR-013).
pub mod account_performance;
/// Account Summary: per-account global value for the Accounts list (ACC-021).
pub mod account_summary;
/// Archive asset: guards archiving against active holdings across bounded contexts (OQ-6).
pub mod archive_asset;
/// Asset price auto-fetch: retrieves prices from Yahoo on launch and user demand (MKT-100+).
pub mod asset_price_fetch;
/// Asset Web Lookup: OpenFIGI search to pre-fill the Add Asset form (WEB).
pub mod asset_web_lookup;
/// What this build can do, read by the interface (MKT-211).
pub mod capabilities;
/// Delete asset: guards hard-deletion against existing transactions.
pub mod delete_asset;
/// Fee generation: lazy catch-up management fee deduction across all active schedules (FEE-040+).
pub mod fee_generation;
/// Global Performance: portfolio-wide performance aggregated in the reference currency (GPF spec).
pub mod global_performance;
/// Holding transaction: unified cross-BC orchestrators for buy/sell/correct/cancel/open (TRX, SEL, CSH).
pub mod holding_transaction;
/// Portfolio Sync: cross-context orchestration for the seven cross-BC sync commands (SYN + CFR, D3).
pub mod portfolio_sync;
/// Price freshness: newest price date among held assets and the last fetch on this device (MKT-200–203).
pub mod price_freshness;
/// Price history backfill of one holding over the period the account held it (MKT-190–199).
pub mod price_history_backfill;
/// Historical exchange-rate backfill for every persisted pair (FXR-110–114).
pub mod rate_history_backfill;
/// Scheduled Price Fetch: daily download of closing prices and exchange rates, even while the app is closed (SPF spec).
pub mod scheduled_fetch;
/// Shared stateless valuation helpers reused across performance/summary use cases — owned by neither.
pub mod shared;
/// Application auto-update: detection, download, and installation (R1–R27).
pub mod update_checker;
