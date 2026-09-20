use crate::context::currency::error::CurrencyError;
use anyhow::Result;
use async_trait::async_trait;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::result::Result as StdResult;

use super::currency_pair::validate_iso4217;

/// A dated rate observation for a directed currency pair (FXR entity).
/// Unique by `(from_currency, to_currency, date)`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CurrencyRate {
    /// ISO 4217 source currency.
    pub from_currency: String,
    /// ISO 4217 target currency.
    pub to_currency: String,
    /// ISO 8601 date `YYYY-MM-DD` of this observation.
    pub date: String,
    /// Units of `to_currency` per one unit of `from_currency`, as i64 micros (ADR-001, FXR-010).
    pub rate: i64,
    /// Provenance of this rate (FXR-100).
    pub source: CurrencyRateSource,
}

impl CurrencyRate {
    /// Creates a new CurrencyRate after full validation (FXR-021/022/023):
    /// - `rate_micros > 0`
    /// - `date` is well-formed ISO 8601 and not in the future
    /// - `from_currency` and `to_currency` are recognised ISO 4217 codes
    /// - `from_currency != to_currency`
    pub fn new(
        from_currency: String,
        to_currency: String,
        date: String,
        rate_micros: i64,
        source: CurrencyRateSource,
    ) -> StdResult<Self, CurrencyError> {
        if rate_micros <= 0 {
            return Err(CurrencyError::NotPositive);
        }
        let parsed = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
            .map_err(|_| CurrencyError::InvalidDateFormat { date: date.clone() })?;
        let today = chrono::Local::now().date_naive();
        if parsed > today {
            return Err(CurrencyError::DateInFuture);
        }
        validate_iso4217(&from_currency)?;
        validate_iso4217(&to_currency)?;
        if from_currency == to_currency {
            return Err(CurrencyError::IdentityPair);
        }
        Ok(Self {
            from_currency,
            to_currency,
            date,
            rate: rate_micros,
            source,
        })
    }

    /// Restores a CurrencyRate from storage without validation.
    pub fn from_storage(
        from_currency: String,
        to_currency: String,
        date: String,
        rate_micros: i64,
        source: CurrencyRateSource,
    ) -> Self {
        Self {
            from_currency,
            to_currency,
            date,
            rate: rate_micros,
            source,
        }
    }
}

/// Provenance qualifier for a CurrencyRate record (FXR-100).
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
pub enum CurrencyRateSource {
    /// User-driven write: manual entry or edit (FXR-101).
    Manual,
    /// Auto-fetched from the Frankfurter provider (FXR-102).
    Frankfurter,
    /// Auto-fetched from the ECB XML feed fallback (FXR-102).
    Ecb,
}

/// Interface for CurrencyRate persistence.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CurrencyRateRepository: Send + Sync {
    /// Upserts a rate: inserts or overwrites by `(from_currency, to_currency, date)`,
    /// latest-write-wins regardless of source (FXR-025, ADR-012).
    async fn upsert_rate(&self, rate: CurrencyRate) -> Result<CurrencyRate>;

    /// Deletes the rate for the given `(from_currency, to_currency, date)`.
    /// Returns `Ok(())` regardless of whether the row existed (no-op when absent).
    async fn delete_rate(&self, from_currency: &str, to_currency: &str, date: &str) -> Result<()>;

    /// Returns all rates for the given pair, ordered by `date` descending (FXR-050).
    async fn list_rates_for_pair(
        &self,
        from_currency: &str,
        to_currency: &str,
    ) -> Result<Vec<CurrencyRate>>;

    /// Returns the rate for the exact `(from_currency, to_currency, date)` key,
    /// or `None` when it does not exist.
    async fn get_by_key(
        &self,
        from_currency: &str,
        to_currency: &str,
        date: &str,
    ) -> Result<Option<CurrencyRate>>;

    /// Returns the most-recent rate for the pair whose date is on or before
    /// `as_of`, or `None` when the pair has no such rate (FXR-035). Used by the
    /// valuation lift to value a foreign holding as of a given date.
    async fn latest_rate_on_or_before(
        &self,
        from_currency: &str,
        to_currency: &str,
        as_of: &str,
    ) -> Result<Option<CurrencyRate>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // FXR-021 — new() rejects rate == 0
    #[test]
    fn new_rejects_zero_rate() {
        let err = CurrencyRate::new(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            0,
            CurrencyRateSource::Manual,
        )
        .unwrap_err();
        assert!(matches!(err, CurrencyError::NotPositive), "got: {err:?}");
    }

    // FXR-021 — new() rejects negative rate
    #[test]
    fn new_rejects_negative_rate() {
        let err = CurrencyRate::new(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            -1,
            CurrencyRateSource::Manual,
        )
        .unwrap_err();
        assert!(matches!(err, CurrencyError::NotPositive), "got: {err:?}");
    }

    // FXR-022 — new() rejects a malformed date
    #[test]
    fn new_rejects_malformed_date() {
        let err = CurrencyRate::new(
            "USD".to_string(),
            "EUR".to_string(),
            "not-a-date".to_string(),
            920_000,
            CurrencyRateSource::Manual,
        )
        .unwrap_err();
        assert!(
            matches!(&err, CurrencyError::InvalidDateFormat { date } if date == "not-a-date"),
            "got: {err:?}"
        );
    }

    // FXR-022 — new() rejects a future date
    #[test]
    fn new_rejects_future_date() {
        let err = CurrencyRate::new(
            "USD".to_string(),
            "EUR".to_string(),
            "2099-12-31".to_string(),
            920_000,
            CurrencyRateSource::Manual,
        )
        .unwrap_err();
        assert!(matches!(err, CurrencyError::DateInFuture), "got: {err:?}");
    }

    // FXR-023 — new() rejects an unknown from_currency
    #[test]
    fn new_rejects_unknown_from_currency() {
        let err = CurrencyRate::new(
            "XX".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            920_000,
            CurrencyRateSource::Manual,
        )
        .unwrap_err();
        assert!(
            matches!(&err, CurrencyError::InvalidCurrency { currency } if currency == "XX"),
            "got: {err:?}"
        );
    }

    // FXR-023 — new() rejects an unknown to_currency
    #[test]
    fn new_rejects_unknown_to_currency() {
        let err = CurrencyRate::new(
            "USD".to_string(),
            "ZZ".to_string(),
            "2026-01-01".to_string(),
            920_000,
            CurrencyRateSource::Manual,
        )
        .unwrap_err();
        assert!(
            matches!(&err, CurrencyError::InvalidCurrency { currency } if currency == "ZZ"),
            "got: {err:?}"
        );
    }

    // FXR-011/023 — new() rejects an identity pair
    #[test]
    fn new_rejects_identity_pair() {
        let err = CurrencyRate::new(
            "EUR".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            920_000,
            CurrencyRateSource::Manual,
        )
        .unwrap_err();
        assert!(matches!(err, CurrencyError::IdentityPair), "got: {err:?}");
    }

    // FXR-021/022/023 — new() accepts a fully valid rate
    #[test]
    fn new_accepts_valid_rate() {
        let rate = CurrencyRate::new(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            920_000,
            CurrencyRateSource::Manual,
        )
        .unwrap();
        assert_eq!(rate.from_currency, "USD");
        assert_eq!(rate.to_currency, "EUR");
        assert_eq!(rate.date, "2026-01-01");
        assert_eq!(rate.rate, 920_000);
        assert_eq!(rate.source, CurrencyRateSource::Manual);
    }

    // from_storage round-trips without validation (future date + identity pair accepted)
    #[test]
    fn from_storage_roundtrips_without_validation() {
        let rate = CurrencyRate::from_storage(
            "XX".to_string(),
            "XX".to_string(),
            "2099-01-01".to_string(),
            -1,
            CurrencyRateSource::Ecb,
        );
        assert_eq!(rate.from_currency, "XX");
        assert_eq!(rate.to_currency, "XX");
        assert_eq!(rate.date, "2099-01-01");
        assert_eq!(rate.rate, -1);
        assert_eq!(rate.source, CurrencyRateSource::Ecb);
    }

    // FXR-100 — CurrencyRateSource::Frankfurter variant exists
    #[test]
    fn source_frankfurter_variant_exists() {
        let rate = CurrencyRate::new(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            920_000,
            CurrencyRateSource::Frankfurter,
        )
        .unwrap();
        assert_eq!(rate.source, CurrencyRateSource::Frankfurter);
    }

    // FXR-100 — CurrencyRateSource::Ecb variant exists
    #[test]
    fn source_ecb_variant_exists() {
        let rate = CurrencyRate::new(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            920_000,
            CurrencyRateSource::Ecb,
        )
        .unwrap();
        assert_eq!(rate.source, CurrencyRateSource::Ecb);
    }
}
