use crate::context::currency::domain::cross_rate::cross_rate_micros;
use crate::context::currency::domain::rate_provider::{
    EurSnapshot, RateHistoryProvider, RateProvider,
};
use crate::context::currency::domain::{
    CurrencyPair, CurrencyPairRepository, CurrencyPairSummary, CurrencyRate,
    CurrencyRateRepository, CurrencyRateSource,
};
use crate::context::currency::error::CurrencyError;
use crate::core::{Event, SideEffectEventBus, BACKEND};
use crate::shared::domain::{Rank, RecordKind, SyncedRecord};
use sqlx::SqliteConnection;
use std::result::Result as StdResult;
use std::sync::Arc;

/// Micros representation of `1.0` — the EUR→EUR identity leg used in cross-rate
/// computation (FXR-080).
const ONE_UNIT_MICROS: i64 = 1_000_000;

/// A resolved conversion rate plus the date of the rate observation it came from
/// (FXR-035/090). `rate_date` is `None` for an identity pair (`from == to`),
/// whose `1.0` rate is synthesized and has no observation date.
#[derive(Debug)]
pub struct ResolvedRate {
    /// Conversion rate in micros (1.0 = 1_000_000), ADR-001.
    pub rate_micros: i64,
    /// ISO date of the rate observation used, or `None` for an identity pair.
    pub rate_date: Option<String>,
}

/// Orchestrates the manual rate CRUD operations for the `currency` bounded context.
pub struct CurrencyService {
    pair_repo: Box<dyn CurrencyPairRepository>,
    rate_repo: Box<dyn CurrencyRateRepository>,
    event_bus: Option<Arc<SideEffectEventBus>>,
    rate_provider: Option<Arc<dyn RateProvider>>,
    rate_history_provider: Option<Arc<dyn RateHistoryProvider>>,
}

impl CurrencyService {
    /// Creates a new CurrencyService with the given repositories.
    pub fn new(
        pair_repo: Box<dyn CurrencyPairRepository>,
        rate_repo: Box<dyn CurrencyRateRepository>,
    ) -> Self {
        Self {
            pair_repo,
            rate_repo,
            event_bus: None,
            rate_provider: None,
            rate_history_provider: None,
        }
    }

    /// Attaches an event bus for side-effect notifications.
    pub fn with_event_bus(mut self, bus: Arc<SideEffectEventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Attaches the external rate provider chain used by `refresh_all_rates`
    /// (ADR-009, FXR-070). Without it, the auto-fetch path is a no-op.
    pub fn with_rate_provider(mut self, provider: Arc<dyn RateProvider>) -> Self {
        self.rate_provider = Some(provider);
        self
    }

    /// Attaches the date-range history provider used by `refresh_all_rates_range`
    /// (SPF-036). Without it, the scheduled-fetch FX backfill is a no-op.
    pub fn with_rate_history_provider(mut self, provider: Arc<dyn RateHistoryProvider>) -> Self {
        self.rate_history_provider = Some(provider);
        self
    }

    /// Publishes `CurrencyRateUpdated` when an event bus is attached (FXR-026).
    fn notify_rate_updated(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::CurrencyRateUpdated);
        }
    }

    /// Publishes `CurrencyPairUpdated` when an event bus is attached (FXR-054, SYN-064).
    fn notify_pair_updated(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::CurrencyPairUpdated);
        }
    }

    /// Idempotently declares a currency pair (FXR-054).
    /// Returns the existing pair if it is already present; creates it otherwise.
    pub async fn declare_currency_pair(
        &self,
        from_currency: String,
        to_currency: String,
    ) -> StdResult<CurrencyPair, CurrencyError> {
        let pair = CurrencyPair::new(from_currency, to_currency)?;
        let pair = self.pair_repo.upsert_pair(pair).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "declare_currency_pair: repository failure");
            CurrencyError::DatabaseError
        })?;
        self.notify_pair_updated();
        Ok(pair)
    }

    /// Stamps `rank` on every currency-owned synced row that has never been ranked (CFR-014,
    /// D6), on the first publish's enrolment transaction (SYN-013). Returns how many rows
    /// were stamped.
    pub async fn stamp_sync_rank(
        &self,
        conn: &mut SqliteConnection,
        rank: &Rank,
    ) -> StdResult<u64, CurrencyError> {
        self.pair_repo
            .stamp_sync_rank(conn, rank)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "stamp_sync_rank: repository failure");
                CurrencyError::DatabaseError
            })
    }

    // -------------------------------------------------------------------------
    // Apply entry points (CFR-017) — merge executor writes; no entry guards run
    // -------------------------------------------------------------------------

    /// The synced record of `kind` this device holds for `identity` (CFR-014), on the apply
    /// transaction's connection; `None` when it holds none.
    pub async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> StdResult<Option<SyncedRecord>, CurrencyError> {
        self.pair_repo
            .synced_record(conn, kind, identity)
            .await
            .map_err(|e| applied_write_error("synced_record", e))
    }

    /// Applies an incoming currency pair verbatim (CFR-017); its identity is its own
    /// natural key (CFR-034), so two devices declaring the same pair produce one record.
    pub async fn apply_currency_pair(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), CurrencyError> {
        let pair: CurrencyPair = synced_content(content)?;
        self.pair_repo
            .apply_pair(conn, &pair, &rank)
            .await
            .map_err(|e| applied_write_error("apply_currency_pair", e))
    }

    /// Applies an incoming currency rate verbatim (CFR-017): the observation merge rule
    /// (CFR-050) has already decided it prevails.
    pub async fn apply_currency_rate(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), CurrencyError> {
        let rate: CurrencyRate = synced_content(content)?;
        self.pair_repo
            .apply_rate(conn, &rate, &rank)
            .await
            .map_err(|e| applied_write_error("apply_currency_rate", e))
    }

    /// Applies an incoming removal of `kind`/`identity` (CFR-017). Currency records have no
    /// children — no cascade applies here (only an account owns others, CFR-030).
    pub async fn apply_removal(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> StdResult<(), CurrencyError> {
        self.pair_repo
            .remove_synced(conn, kind, identity)
            .await
            .map_err(|e| applied_write_error("apply_removal", e))
    }

    /// SYN-083 — discards every currency pair and rate this installation holds, on the
    /// rebuild transaction's connection, before the shared history's replace them.
    pub async fn discard_pairs_and_rates(
        &self,
        conn: &mut SqliteConnection,
    ) -> StdResult<(), CurrencyError> {
        self.pair_repo
            .discard_pairs_and_rates(conn)
            .await
            .map_err(|e| applied_write_error("discard_pairs_and_rates", e))
    }

    /// Records a rate for a pair, ensuring the pair exists first (FXR-013 ergonomics).
    /// Sets `source = Manual` (FXR-101). Upserts by `(from, to, date)` (FXR-025).
    /// Publishes `CurrencyRateUpdated` on success (FXR-026).
    pub async fn record_currency_rate(
        &self,
        from_currency: String,
        to_currency: String,
        date: String,
        rate_micros: i64,
    ) -> StdResult<CurrencyRate, CurrencyError> {
        let rate = CurrencyRate::new(
            from_currency,
            to_currency,
            date,
            rate_micros,
            CurrencyRateSource::Manual,
        )?;

        // The rate's currencies are already validated by `CurrencyRate::new`, so the
        // pair is reconstructed without re-validation (FXR-013 ensure-pair side-effect).
        let pair = CurrencyPair::from_storage(rate.from_currency.clone(), rate.to_currency.clone());
        self.pair_repo.upsert_pair(pair).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "record_currency_rate: pair upsert failure");
            CurrencyError::DatabaseError
        })?;

        let saved = self.rate_repo.upsert_rate(rate).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "record_currency_rate: rate upsert failure");
            CurrencyError::DatabaseError
        })?;

        self.notify_rate_updated();
        Ok(saved)
    }

    /// Updates an existing rate (FXR-052).
    ///
    /// - Same date: in-place overwrite of the rate value.
    /// - Different date: deletes the original record and upserts at the new date.
    ///
    /// Returns `RateNotFound` when the original `(from, to, original_date)` does not exist.
    /// Sets `source = Manual` (FXR-101). Publishes `CurrencyRateUpdated` on success (FXR-052).
    pub async fn update_currency_rate(
        &self,
        from_currency: String,
        to_currency: String,
        original_date: String,
        new_date: String,
        new_rate_micros: i64,
    ) -> StdResult<(), CurrencyError> {
        self.rate_repo
            .get_by_key(&from_currency, &to_currency, &original_date)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "update_currency_rate: get_by_key failure");
                CurrencyError::DatabaseError
            })?
            .ok_or_else(|| CurrencyError::RateNotFound {
                from_currency: from_currency.clone(),
                to_currency: to_currency.clone(),
                date: original_date.clone(),
            })?;

        let new_rate = CurrencyRate::new(
            from_currency.clone(),
            to_currency.clone(),
            new_date.clone(),
            new_rate_micros,
            CurrencyRateSource::Manual,
        )?;

        if original_date != new_date {
            self.rate_repo
                .delete_rate(&from_currency, &to_currency, &original_date)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, err = ?e, "update_currency_rate: delete_rate failure");
                    CurrencyError::DatabaseError
                })?;
        }

        self.rate_repo.upsert_rate(new_rate).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "update_currency_rate: upsert_rate failure");
            CurrencyError::DatabaseError
        })?;

        self.notify_rate_updated();
        Ok(())
    }

    /// Deletes the rate at `(from_currency, to_currency, date)` (FXR-053).
    /// Returns `RateNotFound` when the record does not exist.
    /// Never removes the pair (FXR-014).
    /// Publishes `CurrencyRateUpdated` on success (FXR-053).
    pub async fn delete_currency_rate(
        &self,
        from_currency: String,
        to_currency: String,
        date: String,
    ) -> StdResult<(), CurrencyError> {
        self.rate_repo
            .get_by_key(&from_currency, &to_currency, &date)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "delete_currency_rate: get_by_key failure");
                CurrencyError::DatabaseError
            })?
            .ok_or_else(|| CurrencyError::RateNotFound {
                from_currency: from_currency.clone(),
                to_currency: to_currency.clone(),
                date: date.clone(),
            })?;

        self.rate_repo
            .delete_rate(&from_currency, &to_currency, &date)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "delete_currency_rate: delete_rate failure");
                CurrencyError::DatabaseError
            })?;

        self.notify_rate_updated();
        Ok(())
    }

    /// Returns all persisted pairs enriched with their most-recent rate (FXR-051).
    pub async fn list_currency_pairs(&self) -> StdResult<Vec<CurrencyPairSummary>, CurrencyError> {
        self.pair_repo
            .list_pairs_with_latest_rate()
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "list_currency_pairs: repository failure");
                CurrencyError::DatabaseError
            })
    }

    /// Returns all rates for the given pair ordered by date descending (FXR-050).
    /// Returns an empty list for an unknown pair — never `RateNotFound`.
    pub async fn list_currency_rates(
        &self,
        from_currency: String,
        to_currency: String,
    ) -> StdResult<Vec<CurrencyRate>, CurrencyError> {
        self.rate_repo
            .list_rates_for_pair(&from_currency, &to_currency)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "list_currency_rates: repository failure");
                CurrencyError::DatabaseError
            })
    }

    /// Resolves only the micros component of the conversion rate; see
    /// [`Self::resolve_rate`] for the full contract (FXR-035).
    pub async fn resolve_rate_micros(
        &self,
        from_currency: &str,
        to_currency: &str,
        as_of: &str,
    ) -> StdResult<Option<i64>, CurrencyError> {
        Ok(self
            .resolve_rate(from_currency, to_currency, as_of)
            .await?
            .map(|resolved| resolved.rate_micros))
    }

    /// Resolves the conversion rate for valuing a holding priced in
    /// `from_currency` into `to_currency`, as of `as_of`, together with the date
    /// of the rate observation used (FXR-035/090). Returns the most-recent rate
    /// on or before that date, or `None` when no usable rate exists (FXR-034). An
    /// identity pair (`from == to`) resolves to `1.0` (1_000_000 micros) with
    /// `rate_date = None`, without touching the repository. Read-only: never
    /// writes or publishes events.
    pub async fn resolve_rate(
        &self,
        from_currency: &str,
        to_currency: &str,
        as_of: &str,
    ) -> StdResult<Option<ResolvedRate>, CurrencyError> {
        if from_currency == to_currency {
            return Ok(Some(ResolvedRate {
                rate_micros: ONE_UNIT_MICROS,
                rate_date: None,
            }));
        }
        let rate = self
            .rate_repo
            .latest_rate_on_or_before(from_currency, to_currency, as_of)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "resolve_rate: repository failure");
                CurrencyError::DatabaseError
            })?;
        Ok(rate.map(|r| ResolvedRate {
            rate_micros: r.rate,
            rate_date: Some(r.date),
        }))
    }

    /// Auto-fetches and stores current rates for every persisted pair (FXR-070–074).
    ///
    /// Ensures each `scope_pairs` entry persists first (FXR-071/013), then refreshes
    /// all persisted pairs from one EUR-base snapshot (FXR-080/081). A pair whose
    /// EUR leg is absent is skipped (FXR-073/083); a total provider failure or an
    /// empty pair set leaves the stored rates untouched and is not an error
    /// (FXR-070/072). Each stored rate publishes `CurrencyRateUpdated` (FXR-074).
    pub async fn refresh_all_rates(
        &self,
        scope_pairs: Vec<CurrencyPair>,
    ) -> StdResult<(), CurrencyError> {
        for pair in scope_pairs {
            self.pair_repo.upsert_pair(pair).await.map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "refresh_all_rates: pair ensure failure");
                CurrencyError::DatabaseError
            })?;
        }

        let pairs = self
            .pair_repo
            .list_pairs_with_latest_rate()
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "refresh_all_rates: list pairs failure");
                CurrencyError::DatabaseError
            })?;
        if pairs.is_empty() {
            return Ok(()); // FXR-072 — nothing to fetch
        }

        let Some(provider) = &self.rate_provider else {
            tracing::warn!(target: BACKEND, "refresh_all_rates: no rate provider configured; skipping fetch");
            return Ok(());
        };

        let snapshot = match provider.fetch_eur_snapshot().await {
            Ok(snapshot) => snapshot,
            Err(e) => {
                // FXR-070 — total external failure: keep cached rates, no error.
                tracing::warn!(target: BACKEND, err = ?e, "refresh_all_rates: all providers failed; keeping cached rates");
                return Ok(());
            }
        };

        let eur_leg = |currency: &str| -> Option<i64> {
            if currency == "EUR" {
                Some(ONE_UNIT_MICROS)
            } else {
                snapshot.rates.get(currency).copied()
            }
        };

        for pair in pairs {
            let Some(rate_micros) =
                cross_rate_micros(eur_leg(&pair.from_currency), eur_leg(&pair.to_currency))
            else {
                // FXR-073/083 — a missing EUR leg makes the pair unfetchable; skip it.
                continue;
            };
            let rate = CurrencyRate::from_storage(
                pair.from_currency,
                pair.to_currency,
                snapshot.date.clone(),
                rate_micros,
                snapshot.source.clone(),
            );
            match self.rate_repo.upsert_rate(rate).await {
                Ok(_) => self.notify_rate_updated(),
                Err(e) => {
                    // FXR-073 — a per-pair write failure is logged and skipped.
                    tracing::warn!(target: BACKEND, err = ?e, "refresh_all_rates: rate upsert failed; skipping pair");
                }
            }
        }
        Ok(())
    }

    /// Backfills dated FX reference rates for every persisted pair over
    /// `[from, to]` (SPF-035/036), mirroring `refresh_all_rates`'s pair-ensure
    /// step and per-pair EUR-leg skip (SPF-038), but writing one dated rate per
    /// day the history provider actually published (SPF-037) instead of a
    /// single latest snapshot. A missing history provider, an empty pair set,
    /// or a total provider failure leaves stored rates untouched — not an
    /// error (mirrors FXR-070/072, SPF-039 — never fails the caller's run).
    pub async fn refresh_all_rates_range(
        &self,
        scope_pairs: Vec<CurrencyPair>,
        from: &str,
        to: &str,
    ) -> StdResult<(), CurrencyError> {
        for pair in scope_pairs {
            self.pair_repo.upsert_pair(pair).await.map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "refresh_all_rates_range: pair ensure failure");
                CurrencyError::DatabaseError
            })?;
        }

        let pairs = self
            .pair_repo
            .list_pairs_with_latest_rate()
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "refresh_all_rates_range: list pairs failure");
                CurrencyError::DatabaseError
            })?;
        if pairs.is_empty() {
            return Ok(());
        }

        let Some(provider) = &self.rate_history_provider else {
            tracing::warn!(target: BACKEND, "refresh_all_rates_range: no history provider configured; skipping fetch");
            return Ok(());
        };

        let snapshots = match provider.fetch_eur_range(from, to).await {
            Ok(snapshots) => snapshots,
            Err(e) => {
                // SPF-039 — total external failure: keep stored rates, no error.
                tracing::warn!(target: BACKEND, err = ?e, "refresh_all_rates_range: history provider failed; keeping stored rates");
                return Ok(());
            }
        };

        self.write_snapshot_rates(&pairs, snapshots).await;
        Ok(())
    }

    /// FXR-110–114 — strict variant of [`Self::refresh_all_rates_range`] for
    /// the user-triggered history backfill: fetches the dated daily series for
    /// every persisted pair over `[from, to]` and returns the number of rate
    /// rows written. A total provider failure is surfaced (FXR-114) instead of
    /// swallowed; per-pair/per-day skips stay silent (FXR-112).
    pub async fn backfill_rates_range(
        &self,
        from: &str,
        to: &str,
    ) -> StdResult<u32, CurrencyError> {
        let pairs = self
            .pair_repo
            .list_pairs_with_latest_rate()
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "backfill_rates_range: list pairs failure");
                CurrencyError::DatabaseError
            })?;
        if pairs.is_empty() {
            return Ok(0);
        }
        let Some(provider) = &self.rate_history_provider else {
            tracing::warn!(target: BACKEND, "backfill_rates_range: no history provider configured");
            return Ok(0);
        };
        let snapshots = provider.fetch_eur_range(from, to).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "backfill_rates_range: history provider unreachable");
            CurrencyError::ProviderUnreachable
        })?;
        Ok(self.write_snapshot_rates(&pairs, snapshots).await)
    }

    /// Writes one cross-rate row per `(pair, published day)` from the EUR
    /// snapshots (FXR-080–083); missing legs and per-row write failures are
    /// skipped silently (SPF-038, FXR-073). Returns the written-row count.
    async fn write_snapshot_rates(
        &self,
        pairs: &[CurrencyPairSummary],
        snapshots: Vec<EurSnapshot>,
    ) -> u32 {
        let mut written: u32 = 0;
        for snapshot in snapshots {
            let eur_leg = |currency: &str| -> Option<i64> {
                if currency == "EUR" {
                    Some(ONE_UNIT_MICROS)
                } else {
                    snapshot.rates.get(currency).copied()
                }
            };
            for pair in pairs {
                let Some(rate_micros) =
                    cross_rate_micros(eur_leg(&pair.from_currency), eur_leg(&pair.to_currency))
                else {
                    // SPF-038 — a missing EUR leg makes the pair unfetchable that day; skip it.
                    continue;
                };
                let rate = CurrencyRate::from_storage(
                    pair.from_currency.clone(),
                    pair.to_currency.clone(),
                    snapshot.date.clone(),
                    rate_micros,
                    snapshot.source.clone(),
                );
                match self.rate_repo.upsert_rate(rate).await {
                    Ok(_) => written += 1,
                    Err(e) => {
                        // A per-pair write failure is logged and skipped (mirrors FXR-073).
                        tracing::warn!(target: BACKEND, err = ?e, "write_snapshot_rates: rate upsert failed; skipping pair");
                    }
                }
            }
        }
        written
    }
}

/// Reads a synced change's content into the record it carries (CFR-017). A payload this
/// build cannot read is an infrastructure failure: logged, surfaced as `DatabaseError`.
fn synced_content<T: serde::de::DeserializeOwned>(content: &str) -> StdResult<T, CurrencyError> {
    serde_json::from_str(content).map_err(|e| {
        tracing::error!(target: BACKEND, err = %e, "synced content: malformed payload");
        CurrencyError::DatabaseError
    })
}

/// Translates a failed applied write into `DatabaseError` after logging it.
fn applied_write_error(context: &'static str, e: anyhow::Error) -> CurrencyError {
    tracing::error!(target: BACKEND, err = ?e, "{context}: repository failure");
    CurrencyError::DatabaseError
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::currency::domain::{
        MockCurrencyPairRepository, MockCurrencyRateRepository,
    };

    fn make_service(
        pair_repo: MockCurrencyPairRepository,
        rate_repo: MockCurrencyRateRepository,
    ) -> CurrencyService {
        CurrencyService::new(Box::new(pair_repo), Box::new(rate_repo))
    }

    fn make_rate(
        from: &str,
        to: &str,
        date: &str,
        micros: i64,
        source: CurrencyRateSource,
    ) -> CurrencyRate {
        CurrencyRate::from_storage(
            from.to_string(),
            to.to_string(),
            date.to_string(),
            micros,
            source,
        )
    }

    fn make_pair(from: &str, to: &str) -> CurrencyPair {
        CurrencyPair::from_storage(from.to_string(), to.to_string())
    }

    // -------------------------------------------------------------------------
    // declare_currency_pair
    // -------------------------------------------------------------------------

    // FXR-054 — declare_currency_pair returns the pair on first declaration
    #[tokio::test]
    async fn declare_currency_pair_returns_pair_on_first_declaration() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        let rate_repo = MockCurrencyRateRepository::new();

        let svc = make_service(pair_repo, rate_repo);
        let result = svc
            .declare_currency_pair("USD".to_string(), "EUR".to_string())
            .await;

        let pair = result.expect("declare_currency_pair should succeed");
        assert_eq!(pair.from_currency, "USD");
        assert_eq!(pair.to_currency, "EUR");
    }

    // FXR-054 — declare_currency_pair is idempotent: calling again returns the existing pair
    #[tokio::test]
    async fn declare_currency_pair_is_idempotent() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        // Simulates the upsert returning the existing pair on a second call.
        pair_repo
            .expect_upsert_pair()
            .times(2)
            .returning(|_| Ok(make_pair("USD", "EUR")));
        let rate_repo = MockCurrencyRateRepository::new();
        let svc = make_service(pair_repo, rate_repo);

        svc.declare_currency_pair("USD".to_string(), "EUR".to_string())
            .await
            .unwrap();
        let result = svc
            .declare_currency_pair("USD".to_string(), "EUR".to_string())
            .await;

        let pair = result.expect("second declare should also succeed");
        assert_eq!(pair.from_currency, "USD");
        assert_eq!(pair.to_currency, "EUR");
    }

    // FXR-023 — declare_currency_pair rejects an invalid currency code
    #[tokio::test]
    async fn declare_currency_pair_rejects_invalid_currency() {
        let pair_repo = MockCurrencyPairRepository::new();
        let rate_repo = MockCurrencyRateRepository::new();
        let svc = make_service(pair_repo, rate_repo);

        let err = svc
            .declare_currency_pair("XX".to_string(), "EUR".to_string())
            .await
            .unwrap_err();

        assert!(
            matches!(&err, CurrencyError::InvalidCurrency { currency } if currency == "XX"),
            "got: {err:?}"
        );
    }

    // FXR-011 — declare_currency_pair rejects an identity pair
    #[tokio::test]
    async fn declare_currency_pair_rejects_identity_pair() {
        let pair_repo = MockCurrencyPairRepository::new();
        let rate_repo = MockCurrencyRateRepository::new();
        let svc = make_service(pair_repo, rate_repo);

        let err = svc
            .declare_currency_pair("EUR".to_string(), "EUR".to_string())
            .await
            .unwrap_err();

        assert!(matches!(err, CurrencyError::IdentityPair), "got: {err:?}");
    }

    // DatabaseError — declare_currency_pair maps repo failure to DatabaseError
    #[tokio::test]
    async fn declare_currency_pair_maps_repo_failure_to_database_error() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_upsert_pair()
            .returning(|_| Err(anyhow::anyhow!("db exploded")));
        let rate_repo = MockCurrencyRateRepository::new();
        let svc = make_service(pair_repo, rate_repo);

        let err = svc
            .declare_currency_pair("USD".to_string(), "EUR".to_string())
            .await
            .unwrap_err();

        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // record_currency_rate
    // -------------------------------------------------------------------------

    // FXR-025/101 — record_currency_rate upserts with source=Manual
    #[tokio::test]
    async fn record_currency_rate_upserts_with_source_manual() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().returning(Ok);

        let svc = make_service(pair_repo, rate_repo);
        let result = svc
            .record_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "2026-01-01".to_string(),
                920_000,
            )
            .await;

        let rate = result.expect("record_currency_rate should succeed");
        assert_eq!(rate.source, CurrencyRateSource::Manual);
        assert_eq!(rate.rate, 920_000);
    }

    // FXR-013 — record_currency_rate ensures the pair exists before writing the rate.
    // The mock's expect_upsert_pair() with times(1) assertion confirms the pair-ensure call occurs.
    #[tokio::test]
    async fn record_currency_rate_ensures_pair_exists_before_writing_rate() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().times(1).returning(Ok);
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().returning(Ok);

        let svc = make_service(pair_repo, rate_repo);
        svc.record_currency_rate(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            920_000,
        )
        .await
        .unwrap();
        // mockall validates the times(1) expectation on drop — test fails if
        // upsert_pair was not called exactly once.
    }

    // FXR-025 — record_currency_rate publishes CurrencyRateUpdated on success
    #[tokio::test]
    async fn record_currency_rate_publishes_currency_rate_updated_event() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().returning(Ok);

        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_service(pair_repo, rate_repo).with_event_bus(Arc::clone(&bus));

        svc.record_currency_rate(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            920_000,
        )
        .await
        .unwrap();

        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), Event::CurrencyRateUpdated);
    }

    // FXR-021 — record_currency_rate rejects a non-positive rate
    #[tokio::test]
    async fn record_currency_rate_rejects_non_positive_rate() {
        let pair_repo = MockCurrencyPairRepository::new();
        let rate_repo = MockCurrencyRateRepository::new();
        let svc = make_service(pair_repo, rate_repo);

        let err = svc
            .record_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "2026-01-01".to_string(),
                0,
            )
            .await
            .unwrap_err();

        assert!(matches!(err, CurrencyError::NotPositive), "got: {err:?}");
    }

    // FXR-022 — record_currency_rate rejects a future date
    #[tokio::test]
    async fn record_currency_rate_rejects_future_date() {
        let pair_repo = MockCurrencyPairRepository::new();
        let rate_repo = MockCurrencyRateRepository::new();
        let svc = make_service(pair_repo, rate_repo);

        let err = svc
            .record_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "2099-12-31".to_string(),
                920_000,
            )
            .await
            .unwrap_err();

        assert!(matches!(err, CurrencyError::DateInFuture), "got: {err:?}");
    }

    // FXR-022 — record_currency_rate rejects a malformed date
    #[tokio::test]
    async fn record_currency_rate_rejects_malformed_date() {
        let pair_repo = MockCurrencyPairRepository::new();
        let rate_repo = MockCurrencyRateRepository::new();
        let svc = make_service(pair_repo, rate_repo);

        let err = svc
            .record_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "not-a-date".to_string(),
                920_000,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(&err, CurrencyError::InvalidDateFormat { date } if date == "not-a-date"),
            "got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // update_currency_rate
    // -------------------------------------------------------------------------

    // FXR-052 — update_currency_rate same-date: in-place overwrite succeeds
    #[tokio::test]
    async fn update_currency_rate_same_date_succeeds() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Manual,
            )))
        });
        // delete_rate must NOT be called when the date is unchanged
        rate_repo.expect_upsert_rate().returning(Ok);

        let svc = make_service(pair_repo, rate_repo);
        let result = svc
            .update_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "2026-01-01".to_string(),
                "2026-01-01".to_string(),
                950_000,
            )
            .await;

        assert!(
            result.is_ok(),
            "same-date update should succeed: {result:?}"
        );
    }

    // FXR-052 — update_currency_rate changed-date: original record deleted, new one upserted
    #[tokio::test]
    async fn update_currency_rate_changed_date_deletes_original_and_upserts_new() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Manual,
            )))
        });
        rate_repo
            .expect_delete_rate()
            .times(1)
            .returning(|_, _, _| Ok(()));
        rate_repo.expect_upsert_rate().times(1).returning(Ok);

        let svc = make_service(pair_repo, rate_repo);
        let result = svc
            .update_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "2026-01-01".to_string(),
                "2026-01-02".to_string(),
                950_000,
            )
            .await;

        assert!(
            result.is_ok(),
            "changed-date update should succeed: {result:?}"
        );
    }

    // FXR-052 — update_currency_rate returns RateNotFound when the original does not exist
    #[tokio::test]
    async fn update_currency_rate_returns_rate_not_found_for_missing_original() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| Ok(None));

        let svc = make_service(pair_repo, rate_repo);
        let err = svc
            .update_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "2026-01-01".to_string(),
                "2026-01-02".to_string(),
                950_000,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                &err,
                CurrencyError::RateNotFound { from_currency, to_currency, date }
                    if from_currency == "USD" && to_currency == "EUR" && date == "2026-01-01"
            ),
            "got: {err:?}"
        );
    }

    // FXR-101 — update_currency_rate sets source=Manual regardless of original source
    #[tokio::test]
    async fn update_currency_rate_sets_source_to_manual() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Frankfurter,
            )))
        });
        rate_repo.expect_upsert_rate().returning(|r| {
            // The upserted rate must carry source=Manual even though the original was Frankfurter.
            assert_eq!(r.source, CurrencyRateSource::Manual);
            Ok(r)
        });

        let svc = make_service(pair_repo, rate_repo);
        svc.update_currency_rate(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            "2026-01-01".to_string(),
            950_000,
        )
        .await
        .unwrap();
    }

    // FXR-052 — update_currency_rate publishes CurrencyRateUpdated on success
    #[tokio::test]
    async fn update_currency_rate_publishes_currency_rate_updated_event() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Manual,
            )))
        });
        rate_repo.expect_upsert_rate().returning(Ok);

        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_service(pair_repo, rate_repo).with_event_bus(Arc::clone(&bus));

        svc.update_currency_rate(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            "2026-01-01".to_string(),
            950_000,
        )
        .await
        .unwrap();

        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), Event::CurrencyRateUpdated);
    }

    // -------------------------------------------------------------------------
    // delete_currency_rate
    // -------------------------------------------------------------------------

    // FXR-053 — delete_currency_rate succeeds when the rate exists
    #[tokio::test]
    async fn delete_currency_rate_succeeds_when_rate_exists() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Manual,
            )))
        });
        rate_repo.expect_delete_rate().returning(|_, _, _| Ok(()));

        let svc = make_service(pair_repo, rate_repo);
        let result = svc
            .delete_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "2026-01-01".to_string(),
            )
            .await;

        assert!(result.is_ok(), "delete should succeed: {result:?}");
    }

    // FXR-053 — delete_currency_rate returns RateNotFound when absent
    #[tokio::test]
    async fn delete_currency_rate_returns_rate_not_found_when_absent() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| Ok(None));

        let svc = make_service(pair_repo, rate_repo);
        let err = svc
            .delete_currency_rate(
                "USD".to_string(),
                "EUR".to_string(),
                "2026-01-01".to_string(),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                &err,
                CurrencyError::RateNotFound { from_currency, to_currency, date }
                    if from_currency == "USD" && to_currency == "EUR" && date == "2026-01-01"
            ),
            "got: {err:?}"
        );
    }

    // FXR-053 — delete_currency_rate publishes CurrencyRateUpdated on success
    #[tokio::test]
    async fn delete_currency_rate_publishes_currency_rate_updated_event() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Manual,
            )))
        });
        rate_repo.expect_delete_rate().returning(|_, _, _| Ok(()));

        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_service(pair_repo, rate_repo).with_event_bus(Arc::clone(&bus));

        svc.delete_currency_rate(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
        )
        .await
        .unwrap();

        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), Event::CurrencyRateUpdated);
    }

    // -------------------------------------------------------------------------
    // list_currency_rates
    // -------------------------------------------------------------------------

    // FXR-050 — list_currency_rates returns rates ordered by date descending
    #[tokio::test]
    async fn list_currency_rates_returns_rates_ordered_by_date_descending() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_list_rates_for_pair().returning(|_, _| {
            Ok(vec![
                make_rate(
                    "USD",
                    "EUR",
                    "2026-01-03",
                    930_000,
                    CurrencyRateSource::Manual,
                ),
                make_rate(
                    "USD",
                    "EUR",
                    "2026-01-02",
                    920_000,
                    CurrencyRateSource::Manual,
                ),
                make_rate(
                    "USD",
                    "EUR",
                    "2026-01-01",
                    910_000,
                    CurrencyRateSource::Manual,
                ),
            ])
        });

        let svc = make_service(pair_repo, rate_repo);
        let rates = svc
            .list_currency_rates("USD".to_string(), "EUR".to_string())
            .await
            .unwrap();

        assert_eq!(rates.len(), 3);
        assert_eq!(rates[0].date, "2026-01-03");
        assert_eq!(rates[1].date, "2026-01-02");
        assert_eq!(rates[2].date, "2026-01-01");
    }

    // FXR-050 — list_currency_rates returns empty list for an unknown pair (never RateNotFound)
    #[tokio::test]
    async fn list_currency_rates_returns_empty_list_for_unknown_pair() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_list_rates_for_pair()
            .returning(|_, _| Ok(vec![]));

        let svc = make_service(pair_repo, rate_repo);
        let result = svc
            .list_currency_rates("USD".to_string(), "EUR".to_string())
            .await;

        let rates = result.expect("should return Ok(empty vec) for unknown pair");
        assert!(rates.is_empty());
    }

    // -------------------------------------------------------------------------
    // list_currency_pairs
    // -------------------------------------------------------------------------

    // FXR-051 — list_currency_pairs delegates to list_pairs_with_latest_rate
    #[tokio::test]
    async fn list_currency_pairs_returns_all_pairs_with_latest_rate() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .returning(|| {
                Ok(vec![
                    CurrencyPairSummary {
                        from_currency: "USD".to_string(),
                        to_currency: "EUR".to_string(),
                        latest_rate: Some(920_000),
                        latest_rate_date: Some("2026-01-01".to_string()),
                        latest_rate_source: Some(CurrencyRateSource::Manual),
                    },
                    CurrencyPairSummary {
                        from_currency: "GBP".to_string(),
                        to_currency: "EUR".to_string(),
                        latest_rate: None,
                        latest_rate_date: None,
                        latest_rate_source: None,
                    },
                ])
            });
        let rate_repo = MockCurrencyRateRepository::new();

        let svc = make_service(pair_repo, rate_repo);
        let summaries = svc.list_currency_pairs().await.unwrap();

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].from_currency, "USD");
        assert_eq!(summaries[0].latest_rate, Some(920_000));
        assert!(summaries[1].latest_rate.is_none());
    }

    // -------------------------------------------------------------------------
    // Infrastructure-failure translation (every map_err → DatabaseError branch)
    // -------------------------------------------------------------------------

    fn db_err() -> anyhow::Error {
        anyhow::anyhow!("db exploded")
    }

    // record_currency_rate: pair upsert failure → DatabaseError
    #[tokio::test]
    async fn record_currency_rate_maps_pair_upsert_failure_to_database_error() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(|_| Err(db_err()));
        let rate_repo = MockCurrencyRateRepository::new();

        let err = make_service(pair_repo, rate_repo)
            .record_currency_rate("USD".into(), "EUR".into(), "2026-01-01".into(), 920_000)
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // record_currency_rate: rate upsert failure → DatabaseError
    #[tokio::test]
    async fn record_currency_rate_maps_rate_upsert_failure_to_database_error() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().returning(|_| Err(db_err()));

        let err = make_service(pair_repo, rate_repo)
            .record_currency_rate("USD".into(), "EUR".into(), "2026-01-01".into(), 920_000)
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // update_currency_rate: get_by_key failure → DatabaseError
    #[tokio::test]
    async fn update_currency_rate_maps_get_by_key_failure_to_database_error() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_get_by_key()
            .returning(|_, _, _| Err(db_err()));

        let err = make_service(pair_repo, rate_repo)
            .update_currency_rate(
                "USD".into(),
                "EUR".into(),
                "2026-01-01".into(),
                "2026-01-01".into(),
                950_000,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // update_currency_rate: upsert failure → DatabaseError (same-date path)
    #[tokio::test]
    async fn update_currency_rate_maps_upsert_failure_to_database_error() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Manual,
            )))
        });
        rate_repo.expect_upsert_rate().returning(|_| Err(db_err()));

        let err = make_service(pair_repo, rate_repo)
            .update_currency_rate(
                "USD".into(),
                "EUR".into(),
                "2026-01-01".into(),
                "2026-01-01".into(),
                950_000,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // update_currency_rate: delete failure (changed-date path) → DatabaseError
    #[tokio::test]
    async fn update_currency_rate_maps_delete_failure_to_database_error() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Manual,
            )))
        });
        rate_repo
            .expect_delete_rate()
            .returning(|_, _, _| Err(db_err()));

        let err = make_service(pair_repo, rate_repo)
            .update_currency_rate(
                "USD".into(),
                "EUR".into(),
                "2026-01-01".into(),
                "2026-01-02".into(),
                950_000,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // delete_currency_rate: get_by_key failure → DatabaseError
    #[tokio::test]
    async fn delete_currency_rate_maps_get_by_key_failure_to_database_error() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_get_by_key()
            .returning(|_, _, _| Err(db_err()));

        let err = make_service(pair_repo, rate_repo)
            .delete_currency_rate("USD".into(), "EUR".into(), "2026-01-01".into())
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // delete_currency_rate: delete failure → DatabaseError
    #[tokio::test]
    async fn delete_currency_rate_maps_delete_failure_to_database_error() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_get_by_key().returning(|_, _, _| {
            Ok(Some(make_rate(
                "USD",
                "EUR",
                "2026-01-01",
                920_000,
                CurrencyRateSource::Manual,
            )))
        });
        rate_repo
            .expect_delete_rate()
            .returning(|_, _, _| Err(db_err()));

        let err = make_service(pair_repo, rate_repo)
            .delete_currency_rate("USD".into(), "EUR".into(), "2026-01-01".into())
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // list_currency_pairs: repo failure → DatabaseError
    #[tokio::test]
    async fn list_currency_pairs_maps_repo_failure_to_database_error() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .returning(|| Err(db_err()));
        let rate_repo = MockCurrencyRateRepository::new();

        let err = make_service(pair_repo, rate_repo)
            .list_currency_pairs()
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // list_currency_rates: repo failure → DatabaseError
    #[tokio::test]
    async fn list_currency_rates_maps_repo_failure_to_database_error() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_list_rates_for_pair()
            .returning(|_, _| Err(db_err()));

        let err = make_service(pair_repo, rate_repo)
            .list_currency_rates("USD".into(), "EUR".into())
            .await
            .unwrap_err();
        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // resolve_rate_micros — FXR-011 / FXR-034 / FXR-035
    // -------------------------------------------------------------------------

    // FXR-011 — identity pair (from == to) returns Ok(Some(1_000_000)) without
    // consulting the repository (0 calls to latest_rate_on_or_before)
    #[tokio::test]
    async fn resolve_rate_micros_identity_pair_returns_one_without_repo_call() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        // The mock will fail the test if latest_rate_on_or_before is called at all.
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(0)
            .returning(|_, _, _| Ok(None));

        let svc = make_service(pair_repo, rate_repo);
        let result = svc.resolve_rate_micros("EUR", "EUR", "2026-06-01").await;

        assert_eq!(
            result.expect("identity resolve should succeed"),
            Some(1_000_000)
        );
    }

    // FXR-035 — rate found: returns Ok(Some(rate.rate))
    #[tokio::test]
    async fn resolve_rate_micros_returns_rate_when_found() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(|_, _, _| {
                Ok(Some(make_rate(
                    "USD",
                    "EUR",
                    "2026-05-30",
                    1_080_000,
                    CurrencyRateSource::Manual,
                )))
            });

        let svc = make_service(pair_repo, rate_repo);
        let result = svc.resolve_rate_micros("USD", "EUR", "2026-06-01").await;

        assert_eq!(result.expect("resolve should succeed"), Some(1_080_000));
    }

    // FXR-034 — repo returns Ok(None): resolve_rate_micros returns Ok(None)
    #[tokio::test]
    async fn resolve_rate_micros_returns_none_when_no_rate_found() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(|_, _, _| Ok(None));

        let svc = make_service(pair_repo, rate_repo);
        let result = svc.resolve_rate_micros("USD", "EUR", "2026-06-01").await;

        assert_eq!(result.expect("resolve should succeed"), None);
    }

    // repo returns Err → mapped to Err(CurrencyError::DatabaseError)
    #[tokio::test]
    async fn resolve_rate_micros_maps_repo_failure_to_database_error() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(|_, _, _| Err(db_err()));

        let svc = make_service(pair_repo, rate_repo);
        let err = svc
            .resolve_rate_micros("USD", "EUR", "2026-06-01")
            .await
            .unwrap_err();

        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // resolve_rate — FXR-035 / FXR-090 (carries the rate observation date)
    // -------------------------------------------------------------------------

    // FXR-090 — a found rate carries its micros AND its observation date.
    #[tokio::test]
    async fn resolve_rate_returns_rate_and_date_when_found() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(|_, _, _| {
                Ok(Some(make_rate(
                    "USD",
                    "EUR",
                    "2026-05-30",
                    1_080_000,
                    CurrencyRateSource::Manual,
                )))
            });

        let svc = make_service(pair_repo, rate_repo);
        let resolved = svc
            .resolve_rate("USD", "EUR", "2026-06-01")
            .await
            .expect("resolve should succeed")
            .expect("a rate should be found");

        assert_eq!(resolved.rate_micros, 1_080_000);
        assert_eq!(resolved.rate_date.as_deref(), Some("2026-05-30"));
    }

    // FXR-090 — an identity pair resolves to 1.0 with no observation date and
    // without touching the repository.
    #[tokio::test]
    async fn resolve_rate_identity_pair_returns_one_with_none_date() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(0)
            .returning(|_, _, _| Ok(None));

        let svc = make_service(pair_repo, rate_repo);
        let resolved = svc
            .resolve_rate("EUR", "EUR", "2026-06-01")
            .await
            .expect("identity resolve should succeed")
            .expect("identity resolves to a rate");

        assert_eq!(resolved.rate_micros, 1_000_000);
        assert_eq!(resolved.rate_date, None);
    }

    // FXR-034 — no usable rate: resolve_rate returns Ok(None).
    #[tokio::test]
    async fn resolve_rate_returns_none_when_no_rate_found() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(|_, _, _| Ok(None));

        let svc = make_service(pair_repo, rate_repo);
        let resolved = svc
            .resolve_rate("USD", "EUR", "2026-06-01")
            .await
            .expect("resolve should succeed");

        assert!(resolved.is_none());
    }

    // repo failure → Err(CurrencyError::DatabaseError).
    #[tokio::test]
    async fn resolve_rate_maps_repo_failure_to_database_error() {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(|_, _, _| Err(db_err()));

        let svc = make_service(pair_repo, rate_repo);
        let err = svc
            .resolve_rate("USD", "EUR", "2026-06-01")
            .await
            .unwrap_err();

        assert!(matches!(err, CurrencyError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // refresh_all_rates — FXR-071/072/073/074/102
    // -------------------------------------------------------------------------

    fn make_service_with_provider(
        pair_repo: MockCurrencyPairRepository,
        rate_repo: MockCurrencyRateRepository,
        provider: Arc<dyn crate::context::currency::domain::rate_provider::RateProvider>,
    ) -> CurrencyService {
        CurrencyService::new(Box::new(pair_repo), Box::new(rate_repo)).with_rate_provider(provider)
    }

    // FXR-072 — empty persisted-pair set (scope_pairs empty, list returns empty) →
    // provider's fetch_eur_snapshot expected 0 times; Ok returned
    #[tokio::test]
    async fn refresh_all_rates_empty_pair_set_does_not_call_provider() {
        use crate::context::currency::domain::rate_provider::MockRateProvider;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| Ok(vec![]));
        let rate_repo = MockCurrencyRateRepository::new();

        let mut mock_provider = MockRateProvider::new();
        mock_provider.expect_fetch_eur_snapshot().times(0);

        let svc = make_service_with_provider(
            pair_repo,
            rate_repo,
            Arc::new(mock_provider)
                as Arc<dyn crate::context::currency::domain::rate_provider::RateProvider>,
        );

        let result = svc.refresh_all_rates(vec![]).await;
        assert!(result.is_ok(), "empty scope must return Ok: {result:?}");
    }

    // FXR-071/074/102 — happy path: scope_pairs=[USD→EUR], list returns [USD→EUR];
    // provider returns snapshot {USD:1_164_600, date:"2026-06-01", source:Frankfurter};
    // upsert_rate called once with CurrencyRate(USD,EUR,"2026-06-01",858_663,Frankfurter);
    // CurrencyRateUpdated published.
    #[tokio::test]
    async fn refresh_all_rates_happy_path_upserts_rate_and_publishes_event() {
        use crate::context::currency::domain::rate_provider::{EurSnapshot, MockRateProvider};
        use std::collections::HashMap;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_upsert_rate()
            .times(1)
            .withf(|r: &CurrencyRate| {
                r.from_currency == "USD"
                    && r.to_currency == "EUR"
                    && r.date == "2026-06-01"
                    && r.rate == 858_663
                    && r.source == CurrencyRateSource::Frankfurter
            })
            .returning(Ok);

        let mut mock_provider = MockRateProvider::new();
        mock_provider
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| {
                Ok(EurSnapshot {
                    date: "2026-06-01".to_string(),
                    rates: HashMap::from([("USD".to_string(), 1_164_600i64)]),
                    source: CurrencyRateSource::Frankfurter,
                })
            });

        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();

        let svc = CurrencyService::new(Box::new(pair_repo), Box::new(rate_repo))
            .with_event_bus(Arc::clone(&bus))
            .with_rate_provider(Arc::new(mock_provider)
                as Arc<dyn crate::context::currency::domain::rate_provider::RateProvider>);

        let result = svc.refresh_all_rates(vec![make_pair("USD", "EUR")]).await;
        assert!(result.is_ok(), "happy path must return Ok: {result:?}");

        assert!(
            rx.changed().await.is_ok(),
            "expected CurrencyRateUpdated event on the bus"
        );
        assert_eq!(*rx.borrow(), Event::CurrencyRateUpdated);
    }

    // FXR-073/083 — missing-leg skip: pair JPY→KRW but snapshot lacks KRW →
    // upsert_rate NOT called for that pair; no error returned
    #[tokio::test]
    async fn refresh_all_rates_missing_leg_skips_pair_without_error() {
        use crate::context::currency::domain::rate_provider::{EurSnapshot, MockRateProvider};
        use std::collections::HashMap;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "JPY".to_string(),
                    to_currency: "KRW".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        // upsert_rate must NOT be called because KRW is absent from the snapshot
        rate_repo.expect_upsert_rate().times(0);

        let mut mock_provider = MockRateProvider::new();
        mock_provider
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| {
                Ok(EurSnapshot {
                    date: "2026-06-01".to_string(),
                    // KRW is absent; JPY is present but the KRW leg is missing
                    rates: HashMap::from([("JPY".to_string(), 185_740_000i64)]),
                    source: CurrencyRateSource::Frankfurter,
                })
            });

        let svc = make_service_with_provider(
            pair_repo,
            rate_repo,
            Arc::new(mock_provider)
                as Arc<dyn crate::context::currency::domain::rate_provider::RateProvider>,
        );

        let result = svc.refresh_all_rates(vec![make_pair("JPY", "KRW")]).await;
        assert!(
            result.is_ok(),
            "missing-leg skip must not surface as error: {result:?}"
        );
    }

    // FXR-070 — provider failure: fetch_eur_snapshot returns Err →
    // upsert_rate is never called; method returns Ok (total-failure degrade)
    #[tokio::test]
    async fn refresh_all_rates_provider_failure_returns_ok_without_upsert() {
        use crate::context::currency::domain::rate_provider::MockRateProvider;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().times(0);

        let mut mock_provider = MockRateProvider::new();
        mock_provider
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("provider unreachable")));

        let svc = make_service_with_provider(
            pair_repo,
            rate_repo,
            Arc::new(mock_provider)
                as Arc<dyn crate::context::currency::domain::rate_provider::RateProvider>,
        );

        let result = svc.refresh_all_rates(vec![make_pair("USD", "EUR")]).await;
        assert!(
            result.is_ok(),
            "provider failure must degrade gracefully (Ok): {result:?}"
        );
    }

    // FXR-071/013 — pair-ensure: scope_pairs=[CHF→EUR] → upsert_pair called for it
    // before listing (the ensure step runs first per FXR-071)
    #[tokio::test]
    async fn refresh_all_rates_ensures_scope_pairs_before_listing() {
        use crate::context::currency::domain::rate_provider::MockRateProvider;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_upsert_pair()
            .times(1)
            .withf(|p: &CurrencyPair| p.from_currency == "CHF" && p.to_currency == "EUR")
            .returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| Ok(vec![]));
        let rate_repo = MockCurrencyRateRepository::new();

        let mut mock_provider = MockRateProvider::new();
        mock_provider.expect_fetch_eur_snapshot().times(0);

        let svc = make_service_with_provider(
            pair_repo,
            rate_repo,
            Arc::new(mock_provider)
                as Arc<dyn crate::context::currency::domain::rate_provider::RateProvider>,
        );

        let result = svc.refresh_all_rates(vec![make_pair("CHF", "EUR")]).await;
        assert!(
            result.is_ok(),
            "pair-ensure path must return Ok: {result:?}"
        );
        // mockall validates the times(1) expectation on drop — test fails if
        // upsert_pair was not called exactly once with CHF→EUR.
    }

    // No provider configured (CRUD-only construction) → refresh is a no-op: pairs
    // are listed but no fetch/upsert happens. Guards the `else` arm of the provider
    // lookup so a misconfigured service degrades quietly rather than panicking.
    #[tokio::test]
    async fn refresh_all_rates_without_provider_is_noop() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().times(0);

        // Built WITHOUT with_rate_provider — no external tier attached.
        let svc = make_service(pair_repo, rate_repo);

        let result = svc.refresh_all_rates(vec![]).await;
        assert!(
            result.is_ok(),
            "no-provider refresh must be a quiet no-op: {result:?}"
        );
    }

    // FXR-073 — a per-pair upsert failure is logged and skipped; the task still
    // returns Ok rather than aborting the remaining pairs.
    #[tokio::test]
    async fn refresh_all_rates_skips_pair_when_upsert_fails() {
        use crate::context::currency::domain::rate_provider::{EurSnapshot, MockRateProvider};
        use std::collections::HashMap;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_upsert_rate()
            .times(1)
            .returning(|_| Err(db_err()));

        let mut mock_provider = MockRateProvider::new();
        mock_provider
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| {
                Ok(EurSnapshot {
                    date: "2026-06-01".to_string(),
                    rates: HashMap::from([("USD".to_string(), 1_164_600i64)]),
                    source: CurrencyRateSource::Frankfurter,
                })
            });

        let svc = make_service_with_provider(
            pair_repo,
            rate_repo,
            Arc::new(mock_provider)
                as Arc<dyn crate::context::currency::domain::rate_provider::RateProvider>,
        );

        let result = svc.refresh_all_rates(vec![make_pair("USD", "EUR")]).await;
        assert!(
            result.is_ok(),
            "a per-pair upsert failure must be skipped, not surfaced: {result:?}"
        );
    }

    // -------------------------------------------------------------------------
    // refresh_all_rates_range — SPF-035/036/037/038/039
    // -------------------------------------------------------------------------

    fn make_service_with_history_provider(
        pair_repo: MockCurrencyPairRepository,
        rate_repo: MockCurrencyRateRepository,
        history_provider: Arc<dyn RateHistoryProvider>,
    ) -> CurrencyService {
        CurrencyService::new(Box::new(pair_repo), Box::new(rate_repo))
            .with_rate_history_provider(history_provider)
    }

    // SPF-035/036 — a 2-day history for a persisted pair writes one dated rate
    // per published day.
    #[tokio::test]
    async fn refresh_all_rates_range_writes_one_dated_rate_per_published_day() {
        use crate::context::currency::domain::rate_provider::MockRateHistoryProvider;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_upsert_rate()
            .times(2)
            .withf(|r: &CurrencyRate| {
                r.from_currency == "USD"
                    && r.to_currency == "EUR"
                    && (r.date == "2026-06-29" || r.date == "2026-06-30")
            })
            .returning(Ok);

        let mut history_provider = MockRateHistoryProvider::new();
        history_provider
            .expect_fetch_eur_range()
            .times(1)
            .returning(|_, _| {
                Ok(vec![
                    make_snapshot("2026-06-29", 1_140_000),
                    make_snapshot("2026-06-30", 1_141_000),
                ])
            });

        let svc = make_service_with_history_provider(
            pair_repo,
            rate_repo,
            Arc::new(history_provider) as Arc<dyn RateHistoryProvider>,
        );

        let result = svc
            .refresh_all_rates_range(vec![make_pair("USD", "EUR")], "2026-06-29", "2026-06-30")
            .await;
        assert!(result.is_ok(), "happy path must return Ok: {result:?}");
    }

    // SPF-037 — a day absent from the provider's history writes nothing for
    // that day; only the published days produce a row.
    #[tokio::test]
    async fn refresh_all_rates_range_writes_nothing_for_an_absent_day() {
        use crate::context::currency::domain::rate_provider::MockRateHistoryProvider;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        // Only ONE published day in the range → exactly one upsert.
        rate_repo.expect_upsert_rate().times(1).returning(Ok);

        let mut history_provider = MockRateHistoryProvider::new();
        history_provider
            .expect_fetch_eur_range()
            .times(1)
            .returning(|_, _| Ok(vec![make_snapshot("2026-06-29", 1_140_000)]));

        let svc = make_service_with_history_provider(
            pair_repo,
            rate_repo,
            Arc::new(history_provider) as Arc<dyn RateHistoryProvider>,
        );

        let result = svc
            .refresh_all_rates_range(vec![make_pair("USD", "EUR")], "2026-06-27", "2026-06-29")
            .await;
        assert!(result.is_ok(), "got: {result:?}");
    }

    // SPF-038 — a pair whose EUR leg is missing on a given day is silently
    // skipped for that day; the run continues.
    #[tokio::test]
    async fn refresh_all_rates_range_skips_pair_missing_eur_leg() {
        use crate::context::currency::domain::rate_provider::{
            EurSnapshot, MockRateHistoryProvider,
        };
        use std::collections::HashMap;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "JPY".to_string(),
                    to_currency: "KRW".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().times(0);

        let mut history_provider = MockRateHistoryProvider::new();
        history_provider
            .expect_fetch_eur_range()
            .times(1)
            .returning(|_, _| {
                Ok(vec![EurSnapshot {
                    date: "2026-06-29".to_string(),
                    rates: HashMap::from([("JPY".to_string(), 185_740_000i64)]),
                    source: CurrencyRateSource::Frankfurter,
                }])
            });

        let svc = make_service_with_history_provider(
            pair_repo,
            rate_repo,
            Arc::new(history_provider) as Arc<dyn RateHistoryProvider>,
        );

        let result = svc
            .refresh_all_rates_range(vec![make_pair("JPY", "KRW")], "2026-06-29", "2026-06-29")
            .await;
        assert!(
            result.is_ok(),
            "missing-leg skip must not surface as error: {result:?}"
        );
    }

    // SPF-039 — a total history-provider failure degrades to Ok (never fails
    // the caller's run); mirrors refresh_all_rates's FXR-070 degrade path.
    #[tokio::test]
    async fn refresh_all_rates_range_provider_failure_returns_ok() {
        use crate::context::currency::domain::rate_provider::MockRateHistoryProvider;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().times(0);

        let mut history_provider = MockRateHistoryProvider::new();
        history_provider
            .expect_fetch_eur_range()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("provider unreachable")));

        let svc = make_service_with_history_provider(
            pair_repo,
            rate_repo,
            Arc::new(history_provider) as Arc<dyn RateHistoryProvider>,
        );

        let result = svc
            .refresh_all_rates_range(vec![make_pair("USD", "EUR")], "2026-06-01", "2026-06-30")
            .await;
        assert!(
            result.is_ok(),
            "provider failure must degrade gracefully (Ok): {result:?}"
        );
    }

    // -------------------------------------------------------------------------
    // backfill_rates_range — FXR-110/112/114 (strict, user-triggered variant)
    // -------------------------------------------------------------------------

    // FXR-114 — a total provider failure is surfaced as ProviderUnreachable,
    // unlike the silent refresh_all_rates_range path.
    #[tokio::test]
    async fn backfill_rates_range_surfaces_provider_unreachable() {
        use crate::context::currency::domain::rate_provider::MockRateHistoryProvider;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().times(0);

        let mut history_provider = MockRateHistoryProvider::new();
        history_provider
            .expect_fetch_eur_range()
            .times(1)
            .returning(|_, _| Err(anyhow::anyhow!("provider unreachable")));

        let svc = make_service_with_history_provider(
            pair_repo,
            rate_repo,
            Arc::new(history_provider) as Arc<dyn RateHistoryProvider>,
        );

        let error = svc
            .backfill_rates_range("2019-01-01", "2026-07-14")
            .await
            .unwrap_err();
        assert!(
            matches!(error, CurrencyError::ProviderUnreachable),
            "got: {error:?}"
        );
    }

    // FXR-112 — the backfill writes one dated row per published day per pair
    // and returns the written count.
    #[tokio::test]
    async fn backfill_rates_range_returns_written_count() {
        use crate::context::currency::domain::rate_provider::MockRateHistoryProvider;
        use std::collections::HashMap;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| {
                Ok(vec![CurrencyPairSummary {
                    from_currency: "USD".to_string(),
                    to_currency: "EUR".to_string(),
                    latest_rate: None,
                    latest_rate_date: None,
                    latest_rate_source: None,
                }])
            });
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo.expect_upsert_rate().times(2).returning(Ok);

        let mut history_provider = MockRateHistoryProvider::new();
        history_provider
            .expect_fetch_eur_range()
            .times(1)
            .withf(|from, to| from == "2019-01-01" && to == "2026-07-14")
            .returning(|_, _| {
                Ok(vec![
                    EurSnapshot {
                        date: "2019-01-02".to_string(),
                        rates: HashMap::from([("USD".to_string(), 1_140_000_i64)]),
                        source: CurrencyRateSource::Frankfurter,
                    },
                    EurSnapshot {
                        date: "2019-01-03".to_string(),
                        rates: HashMap::from([("USD".to_string(), 1_142_000_i64)]),
                        source: CurrencyRateSource::Frankfurter,
                    },
                ])
            });

        let svc = make_service_with_history_provider(
            pair_repo,
            rate_repo,
            Arc::new(history_provider) as Arc<dyn RateHistoryProvider>,
        );

        let written = svc
            .backfill_rates_range("2019-01-01", "2026-07-14")
            .await
            .unwrap();
        assert_eq!(written, 2);
    }

    // FXR-111 — no persisted pair means nothing to backfill: quiet zero.
    #[tokio::test]
    async fn backfill_rates_range_with_no_pairs_is_a_quiet_zero() {
        use crate::context::currency::domain::rate_provider::MockRateHistoryProvider;

        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_list_pairs_with_latest_rate()
            .times(1)
            .returning(|| Ok(vec![]));
        let rate_repo = MockCurrencyRateRepository::new();
        let mut history_provider = MockRateHistoryProvider::new();
        history_provider.expect_fetch_eur_range().times(0);

        let svc = make_service_with_history_provider(
            pair_repo,
            rate_repo,
            Arc::new(history_provider) as Arc<dyn RateHistoryProvider>,
        );

        let written = svc
            .backfill_rates_range("2019-01-01", "2026-07-14")
            .await
            .unwrap();
        assert_eq!(written, 0);
    }

    // -------------------------------------------------------------------------
    // CFR-017 — apply entry points bypass entry guards
    // -------------------------------------------------------------------------

    use crate::shared::domain::{LogicalTimestamp, Origin};

    fn incoming_rank(device_id: &str, timestamp: u64) -> Rank {
        Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(timestamp),
            device_id: device_id.to_string(),
        }
    }

    /// A connection the apply entry points write through (SYN-065) — the repositories
    /// behind them are mocked, so the connection itself is never touched.
    async fn apply_conn() -> sqlx::pool::PoolConnection<sqlx::Sqlite> {
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool")
            .acquire()
            .await
            .expect("connection")
    }

    // CFR-017/CFR-034 — apply_currency_pair writes the incoming pair verbatim.
    #[tokio::test]
    async fn apply_currency_pair_writes_incoming_pair_verbatim() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_apply_pair()
            .withf(|_, pair, rank| {
                pair.from_currency == "USD"
                    && pair.to_currency == "EUR"
                    && rank.device_id == "laptop"
            })
            .times(1)
            .returning(|_, _, _| Ok(()));
        let svc = CurrencyService::new(
            Box::new(pair_repo),
            Box::new(MockCurrencyRateRepository::new()),
        );
        let content = r#"{"from_currency":"USD","to_currency":"EUR"}"#;
        svc.apply_currency_pair(
            &mut *apply_conn().await,
            content,
            incoming_rank("laptop", 100),
        )
        .await
        .expect("CFR-017: applying an incoming currency pair must succeed");
    }

    // FXR-054 — declaring a pair announces it, so a view listing pairs can refresh
    // precisely instead of on the bare SyncCompleted marker.
    #[tokio::test]
    async fn declare_currency_pair_publishes_currency_pair_updated() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo.expect_upsert_pair().returning(Ok);
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_service(pair_repo, MockCurrencyRateRepository::new())
            .with_event_bus(Arc::clone(&bus));

        svc.declare_currency_pair("USD".to_string(), "EUR".to_string())
            .await
            .expect("FXR-054: declaring a valid pair succeeds");

        tokio::time::timeout(std::time::Duration::from_millis(200), rx.changed())
            .await
            .expect("CurrencyPairUpdated not received within 200ms")
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::CurrencyPairUpdated);
    }

    // FXR-054 — a rejected pair announces nothing: the bus never fires.
    #[tokio::test]
    async fn declare_currency_pair_publishes_nothing_when_rejected() {
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_service(
            MockCurrencyPairRepository::new(),
            MockCurrencyRateRepository::new(),
        )
        .with_event_bus(Arc::clone(&bus));

        svc.declare_currency_pair("EUR".to_string(), "EUR".to_string())
            .await
            .expect_err("FXR-054: an identity pair is rejected");

        // 50 ms on purpose: this waits for a notice that must never come.
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), rx.changed())
                .await
                .is_err(),
            "a rejected declaration must not announce a pair"
        );
    }

    // SYN-064 — applying a pair publishes nothing: the sync run announces everything it
    // applied with one SyncCompleted after its transaction commits.
    #[tokio::test]
    async fn apply_currency_pair_publishes_no_event() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_apply_pair()
            .times(1)
            .returning(|_, _, _| Ok(()));
        let bus = Arc::new(SideEffectEventBus::new());
        let rx = bus.subscribe();
        let svc = make_service(pair_repo, MockCurrencyRateRepository::new())
            .with_event_bus(Arc::clone(&bus));

        svc.apply_currency_pair(
            &mut *apply_conn().await,
            r#"{"from_currency":"USD","to_currency":"EUR"}"#,
            incoming_rank("laptop", 100),
        )
        .await
        .expect("CFR-017: applying an incoming currency pair must succeed");

        assert!(
            !rx.has_changed().unwrap(),
            "SYN-064: an applied pair must not raise its own event"
        );
    }

    // SYN-064 — an applied removal publishes nothing either.
    #[tokio::test]
    async fn apply_removal_publishes_no_event() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_remove_synced()
            .times(2)
            .returning(|_, _, _| Ok(()));
        let bus = Arc::new(SideEffectEventBus::new());
        let rx = bus.subscribe();
        let svc = make_service(pair_repo, MockCurrencyRateRepository::new())
            .with_event_bus(Arc::clone(&bus));

        for (kind, identity) in [
            (RecordKind::CurrencyPair, "USD|EUR"),
            (RecordKind::CurrencyRate, "USD|EUR|2026-08-21"),
        ] {
            svc.apply_removal(&mut *apply_conn().await, kind, identity)
                .await
                .expect("CFR-017: applying a removal must succeed");
        }

        assert!(
            !rx.has_changed().unwrap(),
            "SYN-064: an applied removal must not raise its own event"
        );
    }

    // CFR-050 — apply_currency_rate writes the observation the engine decided prevails; the
    // service itself runs no rank check.
    #[tokio::test]
    async fn apply_currency_rate_merges_by_observation_rule() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_apply_rate()
            .withf(|_, rate, _| rate.date == "2026-08-21" && rate.rate == 920_000)
            .times(1)
            .returning(|_, _, _| Ok(()));
        let svc = CurrencyService::new(
            Box::new(pair_repo),
            Box::new(MockCurrencyRateRepository::new()),
        );
        let content = r#"{"from_currency":"USD","to_currency":"EUR","date":"2026-08-21","rate":920000,"source":"Manual"}"#;
        svc.apply_currency_rate(
            &mut *apply_conn().await,
            content,
            incoming_rank("laptop", 1_300),
        )
        .await
        .expect("CFR-050: an observation always applies by timestamp, no rank check");
    }

    // CFR-017 — apply_removal of a currency pair applies with no cascade (currency records
    // have no children).
    #[tokio::test]
    async fn apply_removal_of_a_currency_pair_succeeds() {
        let mut pair_repo = MockCurrencyPairRepository::new();
        pair_repo
            .expect_remove_synced()
            .withf(|_, kind, identity| *kind == RecordKind::CurrencyPair && identity == "USD:EUR")
            .times(1)
            .returning(|_, _, _| Ok(()));
        let svc = CurrencyService::new(
            Box::new(pair_repo),
            Box::new(MockCurrencyRateRepository::new()),
        );
        svc.apply_removal(
            &mut *apply_conn().await,
            RecordKind::CurrencyPair,
            "USD:EUR",
        )
        .await
        .expect("CFR-017: an incoming currency-pair removal must apply");
    }

    fn make_snapshot(
        date: &str,
        usd_rate: i64,
    ) -> crate::context::currency::domain::rate_provider::EurSnapshot {
        crate::context::currency::domain::rate_provider::EurSnapshot {
            date: date.to_string(),
            rates: std::collections::HashMap::from([("USD".to_string(), usd_rate)]),
            source: CurrencyRateSource::Frankfurter,
        }
    }
}
