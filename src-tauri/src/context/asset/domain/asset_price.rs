use crate::context::asset::error::AssetError;
use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::result::Result as StdResult;

/// A recorded market price for a financial asset on a specific date.
/// Owned by the `asset` bounded context (MKT spec).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AssetPrice {
    /// ID of the asset whose market price this record describes.
    pub asset_id: String,
    /// ISO 8601 calendar date of the price observation (e.g. "2026-04-26").
    pub date: String,
    /// Market price per unit in the asset's native currency (i64 micro-units, ADR-001).
    pub price: i64,
    /// Provenance of this price record (MKT-100).
    pub source: AssetPriceSource,
}

impl AssetPrice {
    // `with_id()` is not applicable: AssetPrice has no surrogate ID.
    // Its identity is the composite natural key (asset_id, date).

    /// Creates a new AssetPrice after validating price > 0 (MKT-021) and
    /// date is well-formed ISO 8601 and not in the future (MKT-022).
    pub fn new(
        asset_id: String,
        date: String,
        price: i64,
        source: AssetPriceSource,
    ) -> StdResult<Self, AssetError> {
        if price <= 0 {
            return Err(AssetError::NotPositive);
        }
        let parsed = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map_err(|_| AssetError::InvalidDateFormat { date: date.clone() })?;
        let today = chrono::Local::now().date_naive();
        if parsed > today {
            return Err(AssetError::DateInFuture);
        }
        Ok(Self {
            asset_id,
            date,
            price,
            source,
        })
    }

    /// Restores an AssetPrice from storage without validation (B1 — restore factory).
    pub fn restore(asset_id: String, date: String, price: i64, source: AssetPriceSource) -> Self {
        Self {
            asset_id,
            date,
            price,
            source,
        }
    }
}

/// Provenance qualifier for an AssetPrice record (MKT-100).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    specta::Type,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum AssetPriceSource {
    /// User-driven write: manual entry or price-history edit (MKT-101).
    Manual,
    /// Fetched from Yahoo Finance by a fetch task or a price history backfill (MKT-102).
    YahooFinance,
}

/// A quote returned by a [`PriceProvider`]: the price plus the provider's
/// observation date (MKT-117) — the date the quote is _for_, not the fetch time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    /// Market price per unit in the asset's native currency (i64 micros, ADR-001).
    pub price: i64,
    /// Observation date as the provider reported it, ISO `yyyy-mm-dd`. `None` when
    /// the provider supplies no date; the dispatcher then falls back to today (MKT-118).
    pub date: Option<String>,
}

/// A single day's closing price (SPF-030) — always a completed trading day's
/// close, never a live/intraday quote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatedClose {
    /// ISO 8601 calendar date this close is for (e.g. "2026-04-26").
    pub date: String,
    /// Closing price per unit in the asset's native currency (i64 micros, ADR-001).
    pub price: i64,
}

/// What a price history backfill recorded over the held period (MKT-194).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct PriceHistoryBackfillOutcome {
    /// Closes recorded on dates that carried no price (MKT-192).
    pub written: u32,
    /// Closes left aside because their date already carried a price.
    pub already_priced: u32,
}

/// External price-data provider trait (MKT-110, ADR-017).
/// Returns the latest price as i64 micros (ADR-001) with its observation date (MKT-117).
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait PriceProvider: Send + Sync {
    /// Fetches the latest quote for the given provider symbol. Keyless (ADR-017):
    /// the provider (Yahoo Finance) requires no credential.
    ///
    /// - `Ok(Some(quote))` — the provider returned a usable price and (optionally) its
    ///   observation date.
    /// - `Ok(None)` — the provider reports "no data" for this symbol (an unknown /
    ///   delisted symbol). Treated as a quiet per-asset skip with a `tracing::debug!`
    ///   line; not a fetch failure.
    /// - `Err(_)` — transient HTTP / parse / IO failure. The dispatcher logs at
    ///   `tracing::warn!` and continues with the next asset (MKT-114).
    async fn fetch_price(&self, symbol: &str) -> anyhow::Result<Option<Quote>>;

    /// Fetches the daily-close series for `symbol` covering `[from, to]`
    /// inclusive (ISO dates), for the scheduled-fetch backfill window
    /// (SPF-030/031) and the price history backfill (MKT-193). Non-trading days
    /// (weekends, holidays) are simply absent from the series (SPF-032) — not an
    /// error.
    ///
    /// - `Ok(Some(closes))` — the provider knows the symbol; the series may be
    ///   empty when the window holds no trading day (MKT-196).
    /// - `Ok(None)` — the provider does not know the symbol (MKT-196).
    /// - `Err(_)` — transport or parse failure: a per-asset skip in the scheduled
    ///   fetch (SPF-041), a rejection in the price history backfill (MKT-195).
    ///
    /// Defaulted so existing single-purpose fakes (e.g. integration-test
    /// providers exercising only `fetch_price`) do not need to implement a
    /// method they never call; `ReqwestYahooClient` overrides it.
    async fn fetch_daily_closes(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<Option<Vec<DatedClose>>> {
        let _ = (from, to);
        Err(anyhow::anyhow!(
            "daily close series not supported by this provider for symbol: {symbol}"
        ))
    }
}

/// Interface for AssetPrice persistence (upsert by (asset_id, date), MKT-025).
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AssetPriceRepository: Send + Sync {
    /// Upserts a price record: inserts or overwrites by (asset_id, date) (MKT-025).
    async fn upsert(&self, price: AssetPrice) -> Result<()>;
    /// Returns the most recently dated price for the given asset, or None (MKT-031).
    async fn get_latest(&self, asset_id: &str) -> Result<Option<AssetPrice>>;
    /// Returns all recorded prices for the given asset, ordered by date descending (MKT-072).
    async fn get_all_for_asset(&self, asset_id: &str) -> Result<Vec<AssetPrice>>;
    /// Returns the price record for the given (asset_id, date) pair, or None (MKT-083).
    async fn get_by_asset_and_date(&self, asset_id: &str, date: &str)
        -> Result<Option<AssetPrice>>;
    /// Deletes the price record for the given (asset_id, date) pair; no-op if absent (MKT-090).
    async fn delete(&self, asset_id: &str, date: &str) -> Result<()>;
    /// Atomically deletes the record at `original_date` and upserts `new_price` (MKT-084).
    async fn replace_atomic(
        &self,
        asset_id: &str,
        original_date: &str,
        new_price: AssetPrice,
    ) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // MKT-100 / MKT-101 — AssetPriceSource enum + new() fourth param
    // These tests fail until AssetPriceSource is declared and new() / restore()
    // gain the `source` parameter.
    // -------------------------------------------------------------------------

    // MKT-100 — AssetPriceSource::Manual variant exists and is the value stamped
    // by the user-driven write path (MKT-101).
    #[test]
    fn new_accepts_source_manual() {
        let ap = AssetPrice::new(
            "asset-1".to_string(),
            "2026-01-01".to_string(),
            100_000_000,
            AssetPriceSource::Manual,
        )
        .unwrap();
        assert_eq!(ap.source, AssetPriceSource::Manual);
    }

    // MKT-100 — AssetPriceSource::YahooFinance variant exists and round-trips through new().
    #[test]
    fn new_accepts_source_yahoo_finance() {
        let ap = AssetPrice::new(
            "asset-1".to_string(),
            "2026-01-01".to_string(),
            100_000_000,
            AssetPriceSource::YahooFinance,
        )
        .unwrap();
        assert_eq!(ap.source, AssetPriceSource::YahooFinance);
    }

    // MKT-100 — restore() round-trips the source field (B7 restore factory).
    #[test]
    fn restore_roundtrips_source() {
        let ap = AssetPrice::restore(
            "x".to_string(),
            "2026-01-01".to_string(),
            1_000_000,
            AssetPriceSource::YahooFinance,
        );
        assert_eq!(ap.source, AssetPriceSource::YahooFinance);
    }

    // MKT-021 — new() rejects price <= 0
    #[test]
    fn new_rejects_non_positive_price() {
        let err = AssetPrice::new(
            "a".to_string(),
            "2026-01-01".to_string(),
            0,
            AssetPriceSource::Manual,
        )
        .unwrap_err();
        assert!(matches!(err, AssetError::NotPositive), "got: {err:?}");
        let err = AssetPrice::new(
            "a".to_string(),
            "2026-01-01".to_string(),
            -1,
            AssetPriceSource::Manual,
        )
        .unwrap_err();
        assert!(matches!(err, AssetError::NotPositive), "got: {err:?}");
    }

    // MKT-022 — new() rejects a malformed date string with the offending input echoed back
    #[test]
    fn new_rejects_malformed_date() {
        let err = AssetPrice::new(
            "a".to_string(),
            "not-a-date".to_string(),
            1_000_000,
            AssetPriceSource::Manual,
        )
        .unwrap_err();
        assert!(
            matches!(&err, AssetError::InvalidDateFormat { date } if date == "not-a-date"),
            "got: {err:?}"
        );
    }

    // MKT-022 — new() rejects a date that is in the future
    #[test]
    fn new_rejects_future_date() {
        let err = AssetPrice::new(
            "a".to_string(),
            "2099-12-31".to_string(),
            1_000_000,
            AssetPriceSource::Manual,
        )
        .unwrap_err();
        assert!(matches!(err, AssetError::DateInFuture), "got: {err:?}");
    }

    // MKT-021/022 — new() accepts a valid past price and date
    #[test]
    fn new_accepts_valid_past_price_and_date() {
        let ap = AssetPrice::new(
            "asset-1".to_string(),
            "2026-01-01".to_string(),
            100_000_000,
            AssetPriceSource::Manual,
        )
        .unwrap();
        assert_eq!(ap.asset_id, "asset-1");
        assert_eq!(ap.date, "2026-01-01");
        assert_eq!(ap.price, 100_000_000);
    }

    // B1 restore — restore() round-trips fields without validation (negative price + future date accepted)
    #[test]
    fn restore_roundtrips_without_validation() {
        let ap = AssetPrice::restore(
            "x".to_string(),
            "2099-01-01".to_string(),
            -1,
            AssetPriceSource::Manual,
        );
        assert_eq!(ap.asset_id, "x");
        assert_eq!(ap.date, "2099-01-01");
        assert_eq!(ap.price, -1);
    }
}
