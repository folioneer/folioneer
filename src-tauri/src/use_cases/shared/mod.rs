//! What several use cases share — owned by none of them. This module is support
//! infrastructure, not a use case: it holds the device's price fetch log, which the fetch
//! use cases write and the price freshness use case reads (ddd-divergences #14), and the
//! stateless cross-context valuation primitives (portfolio value as of a
//! date, the calendar period series, FX-rate pre-resolution, Simple Dietz metrics)
//! and the single-account performance series engine that `account_performance`,
//! `account_summary` and `global_performance` compose, so no use case has to
//! import from another (B18).

/// Global Value arithmetic (PMV-023) shared by `account_summary` and the Price
/// Movement report so the two never diverge into separate valuation paths.
pub mod global_value;
/// Derived holding inconsistency (CFR-042/SYN-040) reused by every account-reading use case.
pub mod inconsistency;
pub mod performance;
/// This installation's price fetch log (MKT-201), written by `asset_price_fetch` and
/// `scheduled_fetch`, read by `price_freshness`.
pub mod price_fetch_log;
/// Price Movement report arithmetic (PMV spec) — pure, synchronous.
pub mod price_movement;
/// Shared fetch-scope builder (SPF-040) reused by `asset_price_fetch` and `scheduled_fetch`.
pub mod scope;
pub mod valuation;
