use crate::context::currency::domain::CurrencyRateSource;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// A EUR-base snapshot returned by an external rate provider (ADR-009, FXR-070).
///
/// `rates` maps an ISO 4217 code to its EUR→currency rate in i64 micros
/// (e.g. USD at 1.1646 → 1_164_600). EUR itself is NOT a key — it is the implicit
/// base (EUR→EUR = 1_000_000 and is supplied by the caller when needed).
#[derive(Debug, Clone)]
pub struct EurSnapshot {
    /// ISO 8601 date this snapshot applies to (e.g. `"2026-06-01"`).
    pub date: String,
    /// EUR-base rates: ISO 4217 code → micros.
    pub rates: HashMap<String, i64>,
    /// Which provider produced this snapshot (FXR-102).
    pub source: CurrencyRateSource,
}

/// Abstraction over an external EUR-base rate provider (ADR-009, FXR-070).
///
/// Implementations: `ReqwestFrankfurterClient` (primary) and `ReqwestEcbClient`
/// (fallback). The `ChainedRateProvider` composes them in order.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait RateProvider: Send + Sync {
    /// Fetches the latest EUR-base snapshot from the provider.
    /// Returns `Err` on any network or parse failure.
    async fn fetch_eur_snapshot(&self) -> Result<EurSnapshot>;
}

/// Date-range EUR-base rate history for the scheduled-fetch backfill window
/// (SPF-036). Only the primary provider (`ReqwestFrankfurterClient`) implements
/// this — the ECB fallback's daily feed is left untouched (its own 90-day
/// window already covers the 30-day backfill cap when the chain falls back).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait RateHistoryProvider: Send + Sync {
    /// Fetches one dated EUR-base snapshot per published day in `[from, to]`
    /// inclusive (ISO dates). Days with no published reference rate (weekends,
    /// ECB holidays) are simply absent from the result (SPF-037) — not an error.
    async fn fetch_eur_range(&self, from: &str, to: &str) -> Result<Vec<EurSnapshot>>;
}
