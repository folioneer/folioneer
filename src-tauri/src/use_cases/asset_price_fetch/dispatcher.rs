use crate::context::asset::{
    Asset, AssetPrice, AssetPriceRepository, AssetPriceSource, PriceProvider,
};
use crate::context::currency::{CurrencyPair, CurrencyService};
use crate::core::event_bus::{Event, UnpricedAsset};
use crate::core::logger::BACKEND;
use crate::core::SideEffectEventBus;
use chrono::NaiveDate;
use std::sync::Arc;
use std::time::Duration;

use super::guard::FetchGuardLease;
use std::collections::{HashMap, HashSet};

use super::movement_capture::PriceMovementCapture;
use crate::use_cases::shared::price_fetch_log::{FetchMomentClock, PriceFetchLogRepository};
use crate::use_cases::shared::price_movement::build_report;

/// Injectable source of "today" so tests can fix the date deterministically.
pub type Clock = Arc<dyn Fn() -> NaiveDate + Send + Sync>;

/// Politeness delay inserted between successive provider requests in a fetch task.
/// Spacing requests out keeps a launch fetch of a large portfolio from looking like
/// an abusive burst to the provider. It shapes request *cadence* only.
const INTER_FETCH_DELAY: Duration = Duration::from_millis(250);

/// Dispatches the background per-asset price-fetch task (MKT-114, MKT-102, MKT-112).
pub struct Dispatcher {
    provider: Arc<dyn PriceProvider>,
    price_repo: Arc<dyn AssetPriceRepository>,
    event_bus: Arc<SideEffectEventBus>,
    currency_service: Arc<CurrencyService>,
    clock: Clock,
    fetch_log: Option<(Arc<dyn PriceFetchLogRepository>, FetchMomentClock)>,
}

impl Dispatcher {
    /// Creates a new Dispatcher.
    pub fn new(
        provider: Arc<dyn PriceProvider>,
        price_repo: Arc<dyn AssetPriceRepository>,
        event_bus: Arc<SideEffectEventBus>,
        currency_service: Arc<CurrencyService>,
        clock: Clock,
    ) -> Self {
        Self {
            provider,
            price_repo,
            event_bus,
            currency_service,
            clock,
            fetch_log: None,
        }
    }

    /// Records the moment, read from `now`, of each fetch task that priced at least one
    /// asset in this installation's fetch log (MKT-201).
    pub fn with_fetch_log(
        mut self,
        fetch_log: Arc<dyn PriceFetchLogRepository>,
        now: FetchMomentClock,
    ) -> Self {
        self.fetch_log = Some((fetch_log, now));
        self
    }

    /// Spawns a Tokio background task that fetches prices for the pre-derived
    /// `(Asset, symbol)` scope, then refreshes FX rates for `fx_pairs` plus all
    /// persisted pairs (FXR-075/076 — same task, same in-flight lease). The `lease`
    /// is moved into the task; its `Drop` releases the in-flight guard at task end,
    /// panic included (MKT-113).
    ///
    /// `movement_capture` is `Some` only for a user-started Global refresh
    /// (PMV-010/015). It is taken unawaited and run at the top of the task —
    /// before any price is written, so the "before" reading is still refresh
    /// start (PMV-020) while the command itself acknowledges immediately
    /// (MKT-130). The task then builds the report from that baseline and the
    /// prices it actually wrote, and carries it on the completion event.
    pub fn spawn(
        self: Arc<Self>,
        scope: Vec<(Asset, String)>,
        fx_pairs: Vec<CurrencyPair>,
        lease: FetchGuardLease,
        movement_capture: Option<Arc<PriceMovementCapture>>,
    ) {
        tokio::spawn(async move {
            let _lease = lease;
            let today = (self.clock)();
            // PMV-020/021 — the "before" reading, taken before the first write.
            let movement_baseline = match movement_capture {
                Some(capture) => {
                    let scope_asset_ids: HashSet<String> =
                        scope.iter().map(|(asset, _)| asset.id.clone()).collect();
                    capture.capture(&scope_asset_ids, today).await
                }
                None => None,
            };
            // MKT-119 — tally the task outcome so the frontend can summarize it.
            let mut ok: u32 = 0;
            let mut skipped: u32 = 0;
            // MKT-180 — announce the task and report progress after every attempt.
            let total = scope.len() as u32;
            self.event_bus
                .publish(Event::AssetPriceFetchProgress { done: 0, total });
            // MKT-170/171 — one entry per skipped asset, for the manual-fill modal.
            let mut unpriced: Vec<UnpricedAsset> = Vec::with_capacity(scope.len());
            // PMV-022/026 — what THIS refresh wrote, and which assets it could not
            // price (MKT-171). Accumulated here so the report never re-reads the
            // database and so a concurrent writer cannot enter the comparison.
            let mut fetched: HashMap<String, AssetPrice> = HashMap::new();
            let mut unpriced_asset_ids: HashSet<String> = HashSet::new();
            for (index, (asset, symbol)) in scope.into_iter().enumerate() {
                // Space out requests after the first to avoid a burst (see
                // INTER_FETCH_DELAY); the provider is hit at most once per asset.
                if index > 0 {
                    tokio::time::sleep(INTER_FETCH_DELAY).await;
                }
                match self.provider.fetch_price(&symbol).await {
                    Ok(Some(quote)) => {
                        let record = AssetPrice::restore(
                            asset.id.clone(),
                            resolve_observation_date(quote.date.as_deref(), today),
                            quote.price,
                            AssetPriceSource::YahooFinance,
                        );
                        let written = record.clone();
                        if let Err(e) = self.price_repo.upsert(record).await {
                            skipped += 1;
                            unpriced_asset_ids.insert(asset.id.clone());
                            unpriced.push(self.unpriced_entry(&asset).await);
                            tracing::warn!(
                                target: BACKEND,
                                asset_id = %asset.id,
                                symbol = %symbol,
                                err = ?e,
                                "asset_price_fetch: upsert failed; skipping (MKT-114)"
                            );
                            continue;
                        }
                        ok += 1;
                        fetched.insert(asset.id.clone(), written);
                        self.event_bus.publish(Event::AssetPriceUpdated);
                    }
                    Ok(None) => {
                        skipped += 1;
                        unpriced_asset_ids.insert(asset.id.clone());
                        unpriced.push(self.unpriced_entry(&asset).await);
                        tracing::debug!(
                            target: BACKEND,
                            asset_id = %asset.id,
                            symbol = %symbol,
                            "asset_price_fetch: provider reports no data for symbol; skipping (MKT-114)"
                        );
                    }
                    Err(e) => {
                        skipped += 1;
                        unpriced_asset_ids.insert(asset.id.clone());
                        unpriced.push(self.unpriced_entry(&asset).await);
                        tracing::warn!(
                            target: BACKEND,
                            asset_id = %asset.id,
                            symbol = %symbol,
                            err = ?e,
                            "asset_price_fetch: provider fetch failed; skipping (MKT-114)"
                        );
                    }
                }
                // MKT-180 — one progress tick per attempted asset (ok or skipped).
                self.event_bus.publish(Event::AssetPriceFetchProgress {
                    done: ok + skipped,
                    total,
                });
            }

            // MKT-201 — a task that priced at least one asset is a fetch on this device,
            // recorded before the completion event so a reader woken by the event finds
            // the new moment. A failed write never fails the task.
            if let Some((fetch_log, now)) = self.fetch_log.as_ref().filter(|_| ok > 0) {
                if let Err(e) = fetch_log.record_completed(now()).await {
                    tracing::warn!(
                        target: BACKEND,
                        err = ?e,
                        "asset_price_fetch: recording the fetch moment failed (MKT-201)"
                    );
                }
            }

            self.event_bus.publish(Event::AssetPriceFetchCompleted {
                ok,
                skipped,
                unpriced,
                movement: movement_baseline
                    .map(|baseline| build_report(baseline, &fetched, &unpriced_asset_ids)),
            });

            // FXR-075/076 — piggyback FX rate refresh on the same task and lease.
            // refresh_all_rates degrades internally (per-pair skips, provider
            // failure → no-op); a returned error is logged without aborting.
            if let Err(e) = self.currency_service.refresh_all_rates(fx_pairs).await {
                tracing::warn!(
                    target: BACKEND,
                    err = ?e,
                    "asset_price_fetch: FX rate refresh failed; prices already fetched"
                );
            }
        });
    }

    /// Builds the [`UnpricedAsset`] entry for a skipped asset (MKT-170), reading its
    /// most recently recorded price via the repository. A lookup error degrades to no
    /// prior price — the entry is for display only and is never authoritative.
    async fn unpriced_entry(&self, asset: &Asset) -> UnpricedAsset {
        let latest = self.price_repo.get_latest(&asset.id).await.ok().flatten();
        UnpricedAsset {
            asset_id: asset.id.clone(),
            name: asset.name.clone(),
            reference: asset.reference.clone(),
            isin: asset.isin.clone(),
            currency: asset.currency.clone(),
            last_price: latest.as_ref().map(|price| price.price),
            last_price_date: latest.map(|price| price.date),
        }
    }
}

/// Resolves the date a fetched price is stored under (MKT-118): the provider's
/// observation date when it is a well-formed ISO `yyyy-mm-dd` not after `today`,
/// otherwise `today`. Always returns a valid, non-future ISO date.
fn resolve_observation_date(provider_date: Option<&str>, today: NaiveDate) -> String {
    provider_date
        .and_then(|raw| NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok())
        .filter(|parsed| *parsed <= today)
        .unwrap_or(today)
        .format("%Y-%m-%d")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::resolve_observation_date;
    use chrono::NaiveDate;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, 7).expect("valid date")
    }

    #[test]
    fn uses_a_valid_past_observation_date() {
        assert_eq!(
            resolve_observation_date(Some("2026-06-05"), today()),
            "2026-06-05"
        );
    }

    #[test]
    fn uses_an_observation_date_equal_to_today() {
        assert_eq!(
            resolve_observation_date(Some("2026-06-07"), today()),
            "2026-06-07"
        );
    }

    #[test]
    fn falls_back_to_today_when_date_absent() {
        assert_eq!(resolve_observation_date(None, today()), "2026-06-07");
    }

    #[test]
    fn falls_back_to_today_when_date_malformed() {
        assert_eq!(
            resolve_observation_date(Some("not-a-date"), today()),
            "2026-06-07"
        );
        assert_eq!(
            resolve_observation_date(Some("2026-13-40"), today()),
            "2026-06-07"
        );
    }

    #[test]
    fn falls_back_to_today_when_date_in_future() {
        assert_eq!(
            resolve_observation_date(Some("2026-06-08"), today()),
            "2026-06-07"
        );
    }
}
