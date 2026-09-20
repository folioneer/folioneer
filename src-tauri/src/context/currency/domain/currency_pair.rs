use super::currency_rate::CurrencyRate;
use crate::context::currency::error::CurrencyError;
use crate::shared::domain::{Rank, RecordKind, SyncedRecord};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqliteConnection;
use std::result::Result as StdResult;

/// A directed currency pair the system follows for valuation (FXR-013/014).
/// `(from_currency, to_currency)` is unique; the two currencies must differ (FXR-011).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CurrencyPair {
    /// ISO 4217 source currency (e.g. `"USD"`).
    pub from_currency: String,
    /// ISO 4217 target currency (e.g. `"EUR"`).
    pub to_currency: String,
}

impl CurrencyPair {
    /// Creates a new CurrencyPair after validating both codes are ISO 4217
    /// and that `from_currency != to_currency` (FXR-023, FXR-011).
    pub fn new(from_currency: String, to_currency: String) -> StdResult<Self, CurrencyError> {
        validate_iso4217(&from_currency)?;
        validate_iso4217(&to_currency)?;
        if from_currency == to_currency {
            return Err(CurrencyError::IdentityPair);
        }
        Ok(Self {
            from_currency,
            to_currency,
        })
    }

    /// Restores a CurrencyPair from storage without validation.
    pub fn from_storage(from_currency: String, to_currency: String) -> Self {
        Self {
            from_currency,
            to_currency,
        }
    }
}

/// Validates that `code` is a recognised ISO 4217 currency code.
/// Returns `CurrencyError::InvalidCurrency` when the code is unknown.
pub fn validate_iso4217(code: &str) -> StdResult<(), CurrencyError> {
    use iso_currency::Currency;
    use std::str::FromStr;
    Currency::from_str(code).map_err(|_| CurrencyError::InvalidCurrency {
        currency: code.to_string(),
    })?;
    Ok(())
}

/// Row returned by `get_currency_pairs` (FXR-051): a pair enriched with its
/// most-recent rate (per FXR-035). The `latest_*` fields are `None` when no
/// rate has been recorded for the pair yet.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CurrencyPairSummary {
    /// ISO 4217 source currency.
    pub from_currency: String,
    /// ISO 4217 target currency.
    pub to_currency: String,
    /// Micros of the most-recent rate; `None` when no rate has been recorded.
    pub latest_rate: Option<i64>,
    /// ISO date of the most-recent rate; `None` when `latest_rate` is `None`.
    pub latest_rate_date: Option<String>,
    /// Provenance of the most-recent rate; `None` when `latest_rate` is `None`.
    pub latest_rate_source: Option<super::currency_rate::CurrencyRateSource>,
}

/// Interface for CurrencyPair persistence.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CurrencyPairRepository: Send + Sync {
    /// Idempotently ensures the pair exists in the database (FXR-013/054).
    /// Returns the pair (existing or newly created).
    async fn upsert_pair(&self, pair: CurrencyPair) -> Result<CurrencyPair>;

    /// Returns all pairs enriched with their most-recent rate (FXR-051/035).
    async fn list_pairs_with_latest_rate(&self) -> Result<Vec<CurrencyPairSummary>>;

    /// Stamps `rank` on every currency pair and currency rate whose rank columns are still
    /// NULL (CFR-014, D6), on `conn` — the first publish's enrolment transaction (SYN-013).
    /// Returns how many rows were stamped.
    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64>;

    /// The synced record of `kind` this device holds for `identity` — its rank and its
    /// content as the change capture serializes it (CFR-014) — on `conn`; `None` when it
    /// holds none. Covers currency pairs and currency rates.
    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>>;
    /// Writes a currency pair verbatim, stamped with `rank`, on `conn` (CFR-017/034).
    async fn apply_pair(
        &self,
        conn: &mut SqliteConnection,
        pair: &CurrencyPair,
        rank: &Rank,
    ) -> Result<()>;
    /// Writes a currency rate verbatim, stamped with `rank`, on `conn` (CFR-050).
    async fn apply_rate(
        &self,
        conn: &mut SqliteConnection,
        rate: &CurrencyRate,
        rank: &Rank,
    ) -> Result<()>;
    /// Removes the synced record of `kind` for `identity`, on `conn`. A no-op when absent.
    async fn remove_synced(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<()>;
    /// SYN-083 — deletes every currency rate and currency pair, on `conn`.
    async fn discard_pairs_and_rates(&self, conn: &mut SqliteConnection) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // FXR-023 — new() rejects an unknown currency code
    #[test]
    fn new_rejects_unknown_from_currency() {
        let err = CurrencyPair::new("XX".to_string(), "EUR".to_string()).unwrap_err();
        assert!(
            matches!(&err, CurrencyError::InvalidCurrency { currency } if currency == "XX"),
            "got: {err:?}"
        );
    }

    // FXR-023 — new() rejects an unknown to_currency
    #[test]
    fn new_rejects_unknown_to_currency() {
        let err = CurrencyPair::new("USD".to_string(), "ZZ".to_string()).unwrap_err();
        assert!(
            matches!(&err, CurrencyError::InvalidCurrency { currency } if currency == "ZZ"),
            "got: {err:?}"
        );
    }

    // FXR-011/023 — new() rejects an identity pair (from == to)
    #[test]
    fn new_rejects_identity_pair() {
        let err = CurrencyPair::new("EUR".to_string(), "EUR".to_string()).unwrap_err();
        assert!(matches!(err, CurrencyError::IdentityPair), "got: {err:?}");
    }

    // FXR-023 — new() accepts a well-formed directed pair
    #[test]
    fn new_accepts_valid_pair() {
        let pair = CurrencyPair::new("USD".to_string(), "EUR".to_string()).unwrap();
        assert_eq!(pair.from_currency, "USD");
        assert_eq!(pair.to_currency, "EUR");
    }

    // from_storage round-trips without validation (invalid code accepted)
    #[test]
    fn from_storage_roundtrips_without_validation() {
        let pair = CurrencyPair::from_storage("XX".to_string(), "XX".to_string());
        assert_eq!(pair.from_currency, "XX");
        assert_eq!(pair.to_currency, "XX");
    }
}
