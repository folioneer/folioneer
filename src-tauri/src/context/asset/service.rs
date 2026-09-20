use super::domain::{
    Asset, AssetCategory, AssetCategoryRepository, AssetClass, AssetPrice, AssetPriceRepository,
    AssetPriceSource, AssetRepository, DatedClose, PriceHistoryBackfillOutcome, SYSTEM_CATEGORY_ID,
};
use super::error::AssetError;
use crate::{
    context::asset::{CreateAssetDTO, UpdateAssetDTO},
    core::{Event, SideEffectEventBus, BACKEND},
    shared::domain::{Rank, RecordKind, SyncedRecord},
};
use anyhow::Result;
use async_trait::async_trait;
use sqlx::SqliteConnection;
use std::collections::HashSet;
use std::result::Result as StdResult;
use std::sync::Arc;

/// Orchestrates business logic for assets, categories, and market prices.
pub struct AssetService {
    asset_repo: Box<dyn AssetRepository>,
    category_repo: Box<dyn AssetCategoryRepository>,
    price_repo: Box<dyn AssetPriceRepository>,
    event_bus: Option<Arc<SideEffectEventBus>>,
}

impl AssetService {
    /// Creates a new AssetService.
    pub fn new(
        asset_repo: Box<dyn AssetRepository>,
        category_repo: Box<dyn AssetCategoryRepository>,
        price_repo: Box<dyn AssetPriceRepository>,
    ) -> Self {
        Self {
            asset_repo,
            category_repo,
            price_repo,
            event_bus: None,
        }
    }

    /// Attaches an event bus for side-effect notifications.
    pub fn with_event_bus(mut self, bus: Arc<SideEffectEventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    // --- Asset Methods ---

    /// Retrieves all active (non-archived) assets.
    ///
    /// Read-only — only infrastructure failures can fire here, so the surface
    /// is the narrow `AssetError` (only `DatabaseError` reachable).
    pub async fn get_all_assets(&self) -> StdResult<Vec<Asset>, AssetError> {
        self.asset_repo.get_all().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "get_all_assets: repository failure");
            AssetError::DatabaseError
        })
    }

    /// Retrieves all assets including archived ones.
    pub async fn get_all_assets_with_archived(&self) -> StdResult<Vec<Asset>, AssetError> {
        self.asset_repo
            .get_all_including_archived()
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "get_all_assets_with_archived: repository failure");
                AssetError::DatabaseError
            })
    }

    /// Stamps `rank` on every asset-owned synced row that has never been ranked (CFR-014,
    /// D6), on the first publish's enrolment transaction (SYN-013). Returns how many rows
    /// were stamped.
    pub async fn stamp_sync_rank(
        &self,
        conn: &mut SqliteConnection,
        rank: &Rank,
    ) -> StdResult<u64, AssetError> {
        self.asset_repo
            .stamp_sync_rank(conn, rank)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, err = ?e, "stamp_sync_rank: repository failure");
                AssetError::DatabaseError
            })
    }

    /// Retrieves a single asset by ID.
    pub async fn get_asset_by_id(&self, asset_id: &str) -> StdResult<Option<Asset>, AssetError> {
        self.asset_repo.get_by_id(asset_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "get_asset_by_id: repository failure");
            AssetError::DatabaseError
        })
    }

    /// Idempotently seeds the system Cash Asset for `currency` and the system
    /// Cash category that hosts it (CSH-010, CSH-011, CSH-017). Safe to call from every
    /// cash-affecting use case — returns the existing asset on subsequent calls.
    ///
    /// Asset id format: `system-cash-{ccy_lower}`. Category id: `system-cash-category`.
    /// Both constants and the id format live in `core::cash` so the account context
    /// can use the same format without crossing a context boundary (B13).
    pub async fn seed_cash_asset(&self, currency: &str) -> Result<Asset> {
        let asset_id = crate::core::cash::system_cash_asset_id(currency);

        if let Some(existing) = self.asset_repo.get_by_id(&asset_id).await? {
            return Ok(existing);
        }

        let category = match self
            .category_repo
            .get_by_id(crate::core::cash::SYSTEM_CASH_CATEGORY_ID)
            .await?
        {
            Some(c) => c,
            None => {
                let cat = AssetCategory::with_id(
                    crate::core::cash::SYSTEM_CASH_CATEGORY_ID.to_string(),
                    crate::core::cash::SYSTEM_CASH_CATEGORY_LABEL.to_string(),
                )?;
                self.category_repo.create(cat).await?
            }
        };

        let asset = Asset::with_id(
            asset_id.clone(),
            format!("Cash {}", currency.to_uppercase()),
            AssetClass::Cash,
            category,
            currency.to_string(),
            1,
            currency.to_uppercase(),
            None,
            false,
            None,
            false,
            false,
        )?;
        let asset = self.asset_repo.create(asset).await?;
        tracing::info!(target: BACKEND, asset_id = %asset.id, currency = %currency, "Seeded Cash Asset");

        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetUpdated);
        }

        Ok(asset)
    }

    /// Creates a new asset and publishes an AssetUpdated event.
    pub async fn create_asset(&self, dto: CreateAssetDTO) -> StdResult<Asset, AssetError> {
        let category = find_category_for_asset_crud(&*self.category_repo, &dto.category_id).await?;

        let asset = Asset::new(
            dto.name,
            dto.class,
            category,
            dto.currency,
            dto.risk_level,
            dto.reference,
            dto.isin,
            dto.exchange,
            dto.interest_bearing,
        )?;

        let asset = self.asset_repo.create(asset).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "create_asset: repository failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset.id, name = %asset.name, "Asset created");

        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetUpdated);
        }

        Ok(asset)
    }

    /// Updates an existing asset. Rejects if the asset is the system Cash Asset
    /// (CSH-016) or archived (R6) — both invariants are enforced inside
    /// `Asset::update_from` on the loaded aggregate (single source of truth).
    pub async fn update_asset(&self, dto: UpdateAssetDTO) -> StdResult<Asset, AssetError> {
        let existing = load_asset_for_crud(&*self.asset_repo, &dto.asset_id).await?;
        let category = find_category_for_asset_crud(&*self.category_repo, &dto.category_id).await?;

        let asset = existing.update_from(
            dto.name,
            dto.class,
            category,
            dto.currency,
            dto.risk_level,
            dto.reference,
            dto.isin,
            dto.exchange,
            dto.interest_bearing,
        )?;

        let asset = self.asset_repo.update(asset).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %dto.asset_id, err = ?e, "update_asset: repository failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset.id, name = %asset.name, "Asset updated");

        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetUpdated);
        }

        Ok(asset)
    }

    /// Archives an asset (reversible — R6). The system-asset invariant
    /// (CSH-016) is enforced inside `Asset::archive`.
    pub async fn archive_asset(&self, asset_id: &str) -> StdResult<(), AssetError> {
        let existing = load_asset_for_crud(&*self.asset_repo, asset_id).await?;
        // Aggregate enforces the invariant; the returned mutated Asset is
        // intentionally discarded — the column-update fast path
        // `repo.archive(id)` handles persistence.
        existing.archive()?;
        self.asset_repo.archive(asset_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "archive_asset: repository failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset_id, "Asset archived");
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetUpdated);
        }
        Ok(())
    }

    /// Unarchives an asset (R18). The system-asset invariant (CSH-016) is
    /// enforced inside `Asset::unarchive`.
    pub async fn unarchive_asset(&self, asset_id: &str) -> StdResult<(), AssetError> {
        let existing = load_asset_for_crud(&*self.asset_repo, asset_id).await?;
        // See `archive_asset` for the rationale on discarding the returned aggregate.
        existing.unarchive()?;
        self.asset_repo.unarchive(asset_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "unarchive_asset: repository failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset_id, "Asset unarchived");
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetUpdated);
        }
        Ok(())
    }

    /// Blocks automated price fetches for an asset (the lock — MKT-150/156,
    /// ADR-014). The system-asset invariant (CSH-016 / MKT-154) is enforced
    /// inside `Asset::block_price_refresh`.
    pub async fn block_price_refresh(&self, asset_id: &str) -> StdResult<(), AssetError> {
        let existing = load_asset_for_crud(&*self.asset_repo, asset_id).await?;
        // See `archive_asset` for the rationale on discarding the returned aggregate.
        existing.block_price_refresh()?;
        self.asset_repo.block_price_refresh(asset_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "block_price_refresh: repository failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset_id, "Asset price refresh blocked");
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetUpdated);
        }
        Ok(())
    }

    /// Re-allows automated price fetches for an asset (MKT-156). The
    /// system-asset invariant (CSH-016 / MKT-154) is enforced inside
    /// `Asset::unblock_price_refresh`.
    pub async fn unblock_price_refresh(&self, asset_id: &str) -> StdResult<(), AssetError> {
        let existing = load_asset_for_crud(&*self.asset_repo, asset_id).await?;
        // See `archive_asset` for the rationale on discarding the returned aggregate.
        existing.unblock_price_refresh()?;
        self.asset_repo.unblock_price_refresh(asset_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "unblock_price_refresh: repository failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset_id, "Asset price refresh unblocked");
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetUpdated);
        }
        Ok(())
    }

    /// Soft-deletes an asset and publishes an AssetUpdated event. The
    /// system-asset invariant (CSH-016) is enforced inside
    /// `Asset::ensure_user_managed` on the loaded aggregate.
    pub async fn delete_asset(&self, asset_id: &str) -> StdResult<(), AssetError> {
        let existing = load_asset_for_crud(&*self.asset_repo, asset_id).await?;
        existing.ensure_user_managed()?;
        self.asset_repo.delete(asset_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "delete_asset: repository failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset_id, "Asset deleted");
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetUpdated);
        }
        Ok(())
    }

    // --- Category Methods ---

    /// Retrieves all non-deleted categories.
    ///
    /// Read-only — the only failure mode is a repository error, surfaced as
    /// `AssetError::DatabaseError` (typed, payload-free per the
    /// gold infra-translation rule). The full diagnostic stays in
    /// `tracing::error!` server-side.
    pub async fn get_all_categories(&self) -> StdResult<Vec<AssetCategory>, AssetError> {
        self.category_repo.get_all().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "get_all_categories: repository failure");
            AssetError::DatabaseError
        })
    }

    /// Retrieves a category by ID, or `None` when no row matches.
    pub async fn get_category_by_id(&self, id: &str) -> Result<Option<AssetCategory>> {
        self.category_repo.get_by_id(id).await
    }

    /// Creates a category and publishes a CategoryUpdated event.
    pub async fn create_category(&self, label: &str) -> StdResult<AssetCategory, AssetError> {
        if find_category_by_name(&*self.category_repo, label)
            .await?
            .is_some()
        {
            return Err(AssetError::DuplicateName);
        }
        let category = AssetCategory::new(label.to_string())?;
        let category = self.category_repo.create(category).await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "create_category: repository failure");
            AssetError::DatabaseError
        })?;
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::CategoryUpdated);
        }
        Ok(category)
    }

    /// Updates a category and publishes a CategoryUpdated event. The
    /// system-category invariant (`SystemReadonly`) is enforced inside
    /// `AssetCategory::update_from` on the loaded aggregate (single source of
    /// truth). `update_from` runs before the uniqueness query so SystemReadonly
    /// takes precedence over DuplicateName when both would apply.
    pub async fn update_category(
        &self,
        id: &str,
        label: &str,
    ) -> StdResult<AssetCategory, AssetError> {
        let existing = load_category_for_crud(&*self.category_repo, id).await?;
        // CFR-035 — the uniqueness rule (CAT-001) binds the label being set: a label that
        // already clashes after a merge is accepted unchanged, a rename into another
        // category's label is rejected.
        let label_changes = !existing.name.eq_ignore_ascii_case(label);
        let candidate = existing.update_from(label.to_string())?;
        if label_changes {
            if let Some(other) =
                find_category_by_name(&*self.category_repo, &candidate.name).await?
            {
                if other.id != id {
                    return Err(AssetError::DuplicateName);
                }
            }
        }
        let category = self.category_repo.update(candidate).await.map_err(|e| {
            tracing::error!(target: BACKEND, category_id = %id, err = ?e, "update_category: repository failure");
            AssetError::DatabaseError
        })?;
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::CategoryUpdated);
        }
        Ok(category)
    }

    /// Reassigns assets to default category, then deletes the category. The
    /// system-category invariant (`SystemProtected`) is enforced inside
    /// `AssetCategory::ensure_deletable` on the loaded aggregate.
    pub async fn delete_category(&self, category_id: &str) -> StdResult<(), AssetError> {
        let existing = load_category_for_crud(&*self.category_repo, category_id).await?;
        existing.ensure_deletable()?;
        self.category_repo
            .reassign_assets_and_delete(category_id, SYSTEM_CATEGORY_ID)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, category_id = %category_id, err = ?e, "delete_category: repository failure");
                AssetError::DatabaseError
            })?;
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::CategoryUpdated);
        }
        Ok(())
    }

    // --- AssetPrice Methods ---

    /// Converts a decimal f64 price to i64 micro-units at the IPC boundary (ADR-001 / MKT-024).
    /// Caller must have already checked `price_f64.is_finite()`.
    fn f64_to_micros(price_f64: f64) -> i64 {
        (price_f64 * 1_000_000.0).round() as i64
    }

    /// Records (or overwrites) a market price for an asset on a given date (MKT-025).
    /// Validates asset exists (MKT-043), is not archived (AST-006), price > 0 (MKT-021),
    /// date not in future (MKT-022). Publishes AssetPriceUpdated on success (MKT-026).
    pub async fn record_asset_price(
        &self,
        asset_id: &str,
        date: &str,
        price_f64: f64,
    ) -> StdResult<(), AssetError> {
        // MKT-043 + AST-006 — reject unknown or archived asset
        ensure_asset_writable_for_price(&*self.asset_repo, asset_id).await?;
        // MKT-024 — convert f64 decimal to i64 micros at the IPC boundary
        if !price_f64.is_finite() {
            return Err(AssetError::NonFinite);
        }
        let price_micros = Self::f64_to_micros(price_f64);
        // MKT-021, MKT-022 — validate via domain entity factory; MKT-101 — user-driven write stamps source = Manual
        let price = AssetPrice::new(
            asset_id.to_string(),
            date.to_string(),
            price_micros,
            AssetPriceSource::Manual,
        )?;
        // MKT-025 — upsert
        self.price_repo.upsert(price).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, date = %date, err = ?e, "record_asset_price: upsert failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset_id, date = %date, "Asset price recorded");
        // MKT-026 — publish bare signal event
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetPriceUpdated);
        }
        Ok(())
    }

    /// Returns the most recently dated market price for the given asset, or None (MKT-031).
    /// No asset-existence check: MKT-031 is a read-only display fallback; an unknown asset
    /// simply returns None, which is indistinguishable from "no price recorded yet".
    pub async fn get_latest_price(&self, asset_id: &str) -> Result<Option<AssetPrice>> {
        self.price_repo.get_latest(asset_id).await
    }

    /// Returns all recorded market prices for the given asset, sorted date descending (MKT-072).
    /// Rejects with `AssetError::AssetNotFound` if the asset does not exist.
    pub async fn get_asset_prices(&self, asset_id: &str) -> StdResult<Vec<AssetPrice>, AssetError> {
        ensure_asset_exists_for_price(&*self.asset_repo, asset_id).await?;
        self.price_repo
            .get_all_for_asset(asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "get_asset_prices: repository failure");
                AssetError::DatabaseError
            })
    }

    /// Updates the date and/or price of an existing price record (MKT-083/084).
    /// Same-date: in-place upsert. Different-date: atomic delete-old + upsert-new (MKT-084).
    /// Publishes AssetPriceUpdated on success (MKT-085).
    pub async fn update_asset_price(
        &self,
        asset_id: &str,
        original_date: &str,
        new_date: &str,
        price_f64: f64,
    ) -> StdResult<(), AssetError> {
        // Input validation runs before the DB existence check (fail-fast on bad inputs, MKT-082).
        // MKT-082 — finite check before micro conversion
        if !price_f64.is_finite() {
            return Err(AssetError::NonFinite);
        }
        let price_micros = Self::f64_to_micros(price_f64);
        // MKT-082 — validate via domain factory (NotPositive, DateInFuture, InvalidDateFormat); MKT-101 — user-driven write stamps source = Manual
        let new_price = AssetPrice::new(
            asset_id.to_string(),
            new_date.to_string(),
            price_micros,
            AssetPriceSource::Manual,
        )?;
        // AST-006 — reject mutation on archived asset
        ensure_asset_writable_for_price(&*self.asset_repo, asset_id).await?;
        // MKT-083 — reject if original record absent
        ensure_price_exists_for(&*self.price_repo, asset_id, original_date).await?;
        if original_date == new_date {
            // Same date: in-place upsert is atomic by primary key; replace_atomic not needed.
            self.price_repo.upsert(new_price).await.map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %asset_id, date = %new_date, err = ?e, "update_asset_price: upsert failure");
                AssetError::DatabaseError
            })?;
        } else {
            self.price_repo
                .replace_atomic(asset_id, original_date, new_price)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, asset_id = %asset_id, from = %original_date, to = %new_date, err = ?e, "update_asset_price: replace_atomic failure");
                    AssetError::DatabaseError
                })?;
        }
        tracing::info!(target: BACKEND, asset_id = %asset_id, from = %original_date, to = %new_date, "Asset price updated");
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetPriceUpdated);
        }
        Ok(())
    }

    /// Deletes a specific price record by (asset_id, date) (MKT-090).
    /// Returns `AssetError::PriceNotFound` if the record does not exist.
    /// Publishes AssetPriceUpdated on success (MKT-091).
    pub async fn delete_asset_price(
        &self,
        asset_id: &str,
        date: &str,
    ) -> StdResult<(), AssetError> {
        // AST-006 — reject mutation on archived asset
        ensure_asset_writable_for_price(&*self.asset_repo, asset_id).await?;
        ensure_price_exists_for(&*self.price_repo, asset_id, date).await?;
        self.price_repo.delete(asset_id, date).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, date = %date, err = ?e, "delete_asset_price: repository failure");
            AssetError::DatabaseError
        })?;
        tracing::info!(target: BACKEND, asset_id = %asset_id, date = %date, "Asset price deleted");
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetPriceUpdated);
        }
        Ok(())
    }

    /// Publishes AssetPriceUpdated without performing any write.
    /// Called by the record_transaction use case after an atomic DB commit (MKT-057, B8).
    pub fn notify_asset_price_updated(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::AssetPriceUpdated);
        }
    }

    /// Upserts a scheduled-fetch daily-close series for `asset_id`: one
    /// per-`(asset, date)` upsert per entry (MKT-025, latest-write-wins per
    /// ADR-012), stamped `source = YahooFinance` (MKT-102, SPF-034). Returns the
    /// count of rows written. `closes` already excludes non-trading days
    /// (SPF-032) — an empty series writes nothing and returns `Ok(0)`. No event
    /// is published — the headless scheduled-fetch path never notifies a
    /// running app (SPF-024).
    pub async fn record_daily_closes(
        &self,
        asset_id: &str,
        closes: Vec<DatedClose>,
    ) -> StdResult<u32, AssetError> {
        let mut written: u32 = 0;
        for close in closes {
            let record = AssetPrice::restore(
                asset_id.to_string(),
                close.date,
                close.price,
                AssetPriceSource::YahooFinance,
            );
            self.price_repo.upsert(record).await.map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "record_daily_closes: upsert failure");
                AssetError::DatabaseError
            })?;
            written += 1;
        }
        Ok(written)
    }

    /// Records the closes of a price history backfill on the dates that carry no
    /// price for `asset_id`, leaving every recorded price unchanged whatever its
    /// source (MKT-192), stamped `source = YahooFinance` (MKT-102). Publishes one
    /// `AssetPriceUpdated` when at least one close was written (MKT-194).
    pub async fn record_missing_daily_closes(
        &self,
        asset_id: &str,
        closes: Vec<DatedClose>,
    ) -> StdResult<PriceHistoryBackfillOutcome, AssetError> {
        let recorded = self.price_repo.get_all_for_asset(asset_id).await.map_err(|e| {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "record_missing_daily_closes: price listing failure");
            AssetError::DatabaseError
        })?;
        let mut priced_dates: HashSet<String> =
            recorded.into_iter().map(|price| price.date).collect();
        let mut fill = PriceHistoryBackfillOutcome {
            written: 0,
            already_priced: 0,
        };
        for close in closes {
            if !priced_dates.insert(close.date.clone()) {
                fill.already_priced += 1;
                continue;
            }
            let record = AssetPrice::restore(
                asset_id.to_string(),
                close.date,
                close.price,
                AssetPriceSource::YahooFinance,
            );
            self.price_repo.upsert(record).await.map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "record_missing_daily_closes: upsert failure");
                AssetError::DatabaseError
            })?;
            fill.written += 1;
        }
        if let Some(bus) = self.event_bus.as_ref().filter(|_| fill.written > 0) {
            bus.publish(Event::AssetPriceUpdated);
        }
        Ok(fill)
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
    ) -> StdResult<Option<SyncedRecord>, AssetError> {
        self.asset_repo
            .synced_record(conn, kind, identity)
            .await
            .map_err(|e| applied_write_error("synced_record", e))
    }

    /// The rank of another category labelled `name` (CFR-035), on the apply transaction's
    /// connection; `None` when none clashes.
    pub async fn clashing_category_name(
        &self,
        conn: &mut SqliteConnection,
        category_id: &str,
        name: &str,
    ) -> StdResult<Option<Rank>, AssetError> {
        self.asset_repo
            .clashing_category_name_rank(conn, category_id, name)
            .await
            .map_err(|e| applied_write_error("clashing_category_name", e))
    }

    /// Ensures the system Cash Asset for `currency` and the Cash category exist (CSH-010,
    /// SYN-027/CFR-033) on the apply transaction's connection — the seeding
    /// `seed_cash_asset` performs, without the event.
    pub async fn ensure_cash_asset(
        &self,
        conn: &mut SqliteConnection,
        currency: &str,
    ) -> StdResult<(), AssetError> {
        let category = AssetCategory::with_id(
            crate::core::cash::SYSTEM_CASH_CATEGORY_ID.to_string(),
            crate::core::cash::SYSTEM_CASH_CATEGORY_LABEL.to_string(),
        )?;
        let asset = Asset::with_id(
            crate::core::cash::system_cash_asset_id(currency),
            format!("Cash {}", currency.to_uppercase()),
            AssetClass::Cash,
            category.clone(),
            currency.to_string(),
            1,
            currency.to_uppercase(),
            None,
            false,
            None,
            false,
            false,
        )?;
        self.asset_repo
            .ensure_seeded(conn, &category, &asset)
            .await
            .map_err(|e| applied_write_error("ensure_cash_asset", e))
    }

    /// Applies an incoming asset change verbatim (CFR-017): an archived asset, a category
    /// pointing at a since-removed category — none of today's entry guards run. Stamps
    /// `rank` on the row (CFR-014).
    pub async fn apply_asset(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), AssetError> {
        let asset: Asset = synced_content(content)?;
        self.asset_repo
            .apply_asset(conn, &asset, &rank)
            .await
            .map_err(|e| applied_write_error("apply_asset", e))
    }

    /// Applies an incoming category change verbatim (CFR-017); CFR-035's duplicate-label
    /// coexistence is the same shape as the account-name guard.
    pub async fn apply_category(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), AssetError> {
        let category: AssetCategory = synced_content(content)?;
        self.asset_repo
            .apply_category(conn, &category, &rank)
            .await
            .map_err(|e| applied_write_error("apply_category", e))
    }

    /// Applies an incoming asset price verbatim (CFR-017): the observation merge rule
    /// (CFR-050) has already decided it prevails.
    pub async fn apply_asset_price(
        &self,
        conn: &mut SqliteConnection,
        content: &str,
        rank: Rank,
    ) -> StdResult<(), AssetError> {
        let price: AssetPrice = synced_content(content)?;
        self.asset_repo
            .apply_asset_price(conn, &price, &rank)
            .await
            .map_err(|e| applied_write_error("apply_asset_price", e))
    }

    /// Applies an incoming removal of `kind`/`identity` (CFR-017/CFR-030): an asset or a
    /// category is removed as it is removed locally, its assets left on the removed
    /// category — shown in the default category on read (CFR-030) — never a cascade
    /// removal (only an account owns others).
    pub async fn apply_removal(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> StdResult<(), AssetError> {
        self.asset_repo
            .remove_synced(conn, kind, identity)
            .await
            .map_err(|e| applied_write_error("apply_removal", e))
    }

    /// SYN-083 — discards every asset price this installation holds, on the rebuild
    /// transaction's connection, before the shared history's observations replace them.
    pub async fn discard_asset_prices(
        &self,
        conn: &mut SqliteConnection,
    ) -> StdResult<(), AssetError> {
        self.asset_repo
            .discard_asset_prices(conn)
            .await
            .map_err(|e| applied_write_error("discard_asset_prices", e))
    }
}

/// Reads a synced change's content into the record it carries (CFR-017). A payload this
/// build cannot read is an infrastructure failure: logged, surfaced as `DatabaseError`.
fn synced_content<T: serde::de::DeserializeOwned>(content: &str) -> StdResult<T, AssetError> {
    serde_json::from_str(content).map_err(|e| {
        tracing::error!(target: BACKEND, err = %e, "synced content: malformed payload");
        AssetError::DatabaseError
    })
}

/// Translates a failed applied write into `DatabaseError` after logging it.
fn applied_write_error(context: &'static str, e: anyhow::Error) -> AssetError {
    tracing::error!(target: BACKEND, err = ?e, "{context}: repository failure");
    AssetError::DatabaseError
}

/// Asset application surface consumed by cross-BC use-case orchestrators.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait AssetServiceContract: Send + Sync {
    /// Retrieves a single asset by ID.
    async fn get_asset_by_id(&self, asset_id: &str) -> StdResult<Option<Asset>, AssetError>;
    /// Idempotently seeds the system Cash Asset for `currency` (CSH-010, CSH-011, CSH-017).
    async fn seed_cash_asset(&self, currency: &str) -> Result<Asset>;
    /// Archives an asset (reversible — R6).
    async fn archive_asset(&self, asset_id: &str) -> StdResult<(), AssetError>;
    /// Soft-deletes an asset and publishes an AssetUpdated event.
    async fn delete_asset(&self, asset_id: &str) -> StdResult<(), AssetError>;
    /// Returns the most recently dated market price for the given asset, or None (MKT-031).
    async fn get_latest_price(&self, asset_id: &str) -> Result<Option<AssetPrice>>;
    /// Returns all recorded market prices for the given asset, sorted date descending (MKT-072).
    async fn get_asset_prices(&self, asset_id: &str) -> StdResult<Vec<AssetPrice>, AssetError>;
    /// Records a daily-close series for an asset with source=YahooFinance (SPF-034); no event published (SPF-024).
    async fn record_daily_closes(
        &self,
        asset_id: &str,
        closes: Vec<DatedClose>,
    ) -> StdResult<u32, AssetError>;
    /// Records backfill closes on the dates without a price, publishing once (MKT-192/194).
    async fn record_missing_daily_closes(
        &self,
        asset_id: &str,
        closes: Vec<DatedClose>,
    ) -> StdResult<PriceHistoryBackfillOutcome, AssetError>;
}

#[async_trait]
impl AssetServiceContract for AssetService {
    async fn get_asset_by_id(&self, asset_id: &str) -> StdResult<Option<Asset>, AssetError> {
        AssetService::get_asset_by_id(self, asset_id).await
    }

    async fn seed_cash_asset(&self, currency: &str) -> Result<Asset> {
        AssetService::seed_cash_asset(self, currency).await
    }

    async fn archive_asset(&self, asset_id: &str) -> StdResult<(), AssetError> {
        AssetService::archive_asset(self, asset_id).await
    }

    async fn delete_asset(&self, asset_id: &str) -> StdResult<(), AssetError> {
        AssetService::delete_asset(self, asset_id).await
    }

    async fn get_latest_price(&self, asset_id: &str) -> Result<Option<AssetPrice>> {
        AssetService::get_latest_price(self, asset_id).await
    }

    async fn get_asset_prices(&self, asset_id: &str) -> StdResult<Vec<AssetPrice>, AssetError> {
        AssetService::get_asset_prices(self, asset_id).await
    }

    async fn record_daily_closes(
        &self,
        asset_id: &str,
        closes: Vec<DatedClose>,
    ) -> StdResult<u32, AssetError> {
        AssetService::record_daily_closes(self, asset_id, closes).await
    }

    async fn record_missing_daily_closes(
        &self,
        asset_id: &str,
        closes: Vec<DatedClose>,
    ) -> StdResult<PriceHistoryBackfillOutcome, AssetError> {
        AssetService::record_missing_daily_closes(self, asset_id, closes).await
    }
}

/// Loads a category by ID for the CRUD family (update / delete). Translates
/// `Ok(None)` into `AssetError::CategoryNotFound { id }` and any
/// repository error into `AssetError::DatabaseError` after
/// preserving the diagnostic chain server-side via `tracing::error!`.
///
/// Parallel to PR 5's `load_account` helper in the account BC. Used by
/// `update_category` and `delete_category`.
async fn load_category_for_crud(
    repo: &dyn AssetCategoryRepository,
    id: &str,
) -> StdResult<AssetCategory, AssetError> {
    match repo.get_by_id(id).await {
        Ok(Some(cat)) => Ok(cat),
        Ok(None) => Err(AssetError::CategoryNotFound { id: id.to_string() }),
        Err(e) => {
            tracing::error!(target: BACKEND, category_id = %id, err = ?e, "load_category_for_crud: repository failure");
            Err(AssetError::DatabaseError)
        }
    }
}

/// CRUD-family parallel to PR 5's `find_account_by_name`. Wraps the
/// `find_by_name` uniqueness pre-check used by `create_category` and
/// `update_category`, translating any repository failure into
/// `AssetError::DatabaseError`.
///
/// Unlike `load_category_for_crud`, `Ok(None)` is the **success** path here
/// (the name is available); the caller decides what to do with `Some(existing)`.
async fn find_category_by_name(
    repo: &dyn AssetCategoryRepository,
    name: &str,
) -> StdResult<Option<AssetCategory>, AssetError> {
    repo.find_by_name(name).await.map_err(|e| {
        tracing::error!(target: BACKEND, name = %name, err = ?e, "find_category_by_name: repository failure");
        AssetError::DatabaseError
    })
}

/// Loads an asset by ID for the CRUD family. Translates `Ok(None)` into
/// `AssetError::AssetNotFound { id }` and any repository error into
/// `AssetError::DatabaseError` after preserving the diagnostic
/// chain via `tracing::error!`.
async fn load_asset_for_crud(
    repo: &dyn AssetRepository,
    asset_id: &str,
) -> StdResult<Asset, AssetError> {
    match repo.get_by_id(asset_id).await {
        Ok(Some(asset)) => Ok(asset),
        Ok(None) => Err(AssetError::AssetNotFound {
            id: asset_id.to_string(),
        }),
        Err(e) => {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "load_asset_for_crud: repository failure");
            Err(AssetError::DatabaseError)
        }
    }
}

/// Cross-aggregate asset-existence check used by read-only price reads
/// (`get_asset_prices`). Translates `Ok(None)` into
/// `AssetError::AssetNotFound { id }` and any repository error into
/// `AssetError::DatabaseError` after preserving the diagnostic chain
/// via `tracing::error!`.
async fn ensure_asset_exists_for_price(
    repo: &dyn AssetRepository,
    asset_id: &str,
) -> StdResult<(), AssetError> {
    match repo.get_by_id(asset_id).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(AssetError::AssetNotFound {
            id: asset_id.to_string(),
        }),
        Err(e) => {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "ensure_asset_exists_for_price: repository failure");
            Err(AssetError::DatabaseError)
        }
    }
}

/// Cross-aggregate writable-asset check used by mutating price commands
/// (`record_asset_price`, `update_asset_price`, `delete_asset_price`). Adds
/// the AST-006 archive guard on top of the existence check: rejects when the
/// asset is archived with `AssetError::Archived`. Read paths
/// keep using `ensure_asset_exists_for_price` since AST-006 only blocks
/// mutations, not reads.
async fn ensure_asset_writable_for_price(
    repo: &dyn AssetRepository,
    asset_id: &str,
) -> StdResult<(), AssetError> {
    match repo.get_by_id(asset_id).await {
        Ok(Some(asset)) => {
            if asset.is_archived {
                Err(AssetError::Archived)
            } else {
                Ok(())
            }
        }
        Ok(None) => Err(AssetError::AssetNotFound {
            id: asset_id.to_string(),
        }),
        Err(e) => {
            tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "ensure_asset_writable_for_price: repository failure");
            Err(AssetError::DatabaseError)
        }
    }
}

/// Price-row existence check used by `update_asset_price` and
/// `delete_asset_price`. Translates `Ok(None)` into
/// `AssetError::PriceNotFound { asset_id, date }` and any
/// repository error into `AssetError::DatabaseError`.
async fn ensure_price_exists_for(
    repo: &dyn AssetPriceRepository,
    asset_id: &str,
    date: &str,
) -> StdResult<(), AssetError> {
    match repo.get_by_asset_and_date(asset_id, date).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(AssetError::PriceNotFound {
            asset_id: asset_id.to_string(),
            date: date.to_string(),
        }),
        Err(e) => {
            tracing::error!(target: BACKEND, asset_id = %asset_id, date = %date, err = ?e, "ensure_price_exists_for: repository failure");
            Err(AssetError::DatabaseError)
        }
    }
}

/// Looks up a category for the asset CRUD path (cross-aggregate dependency in
/// `create_asset` / `update_asset`). Translates `Ok(None)` into
/// `AssetError::CategoryNotFound { id }` and any repo error into
/// `AssetError::DatabaseError`.
async fn find_category_for_asset_crud(
    repo: &dyn AssetCategoryRepository,
    category_id: &str,
) -> StdResult<AssetCategory, AssetError> {
    match repo.get_by_id(category_id).await {
        Ok(Some(cat)) => Ok(cat),
        Ok(None) => Err(AssetError::CategoryNotFound {
            id: category_id.to_string(),
        }),
        Err(e) => {
            tracing::error!(target: BACKEND, category_id = %category_id, err = ?e, "find_category_for_asset_crud: repository failure");
            Err(AssetError::DatabaseError)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::asset::{
        AssetClass, AssetError, CreateAssetDTO, MockAssetCategoryRepository,
        MockAssetPriceRepository, MockAssetRepository,
    };
    use std::sync::Arc;
    use std::time::Duration;

    fn make_svc(
        ar: MockAssetRepository,
        cr: MockAssetCategoryRepository,
        pr: MockAssetPriceRepository,
    ) -> AssetService {
        AssetService::new(Box::new(ar), Box::new(cr), Box::new(pr))
    }

    fn make_asset(id: &str, archived: bool) -> Asset {
        Asset::restore(
            id.to_string(),
            "Test Asset".to_string(),
            AssetClass::Stocks,
            make_category(),
            "USD".to_string(),
            1,
            "REF".to_string(),
            None,
            archived,
            None,
            false,
            false,
        )
    }

    fn make_cash_asset(id: &str) -> Asset {
        Asset::restore(
            id.to_string(),
            "Cash".to_string(),
            AssetClass::Cash,
            make_category(),
            "USD".to_string(),
            1,
            "USD".to_string(),
            None,
            false,
            None,
            false,
            false,
        )
    }

    fn make_category() -> AssetCategory {
        AssetCategory::from_storage(
            SYSTEM_CATEGORY_ID.to_string(),
            "generic.uncategorized".to_string(),
        )
    }

    fn make_price(asset_id: &str, date: &str, price: i64) -> AssetPrice {
        AssetPrice::restore(
            asset_id.to_string(),
            date.to_string(),
            price,
            AssetPriceSource::Manual,
        )
    }

    /// Mock asset_repo wired to satisfy `ensure_asset_writable_for_price` for
    /// the price-mutation tests: returns a non-archived asset on `get_by_id`.
    fn ar_with_writable_asset(id: &'static str) -> MockAssetRepository {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(move |_| Ok(Some(make_asset(id, false))));
        ar
    }

    fn base_dto(name: &str) -> CreateAssetDTO {
        CreateAssetDTO {
            name: name.to_string(),
            reference: "REF-001".to_string(),
            isin: None,
            class: AssetClass::Cash,
            currency: "USD".to_string(),
            risk_level: 1,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        }
    }

    // R1 — empty name is rejected
    #[tokio::test]
    async fn test_create_asset_rejects_empty_name() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .create_asset(CreateAssetDTO {
                name: "".to_string(),
                ..base_dto("ignored")
            })
            .await
            .unwrap_err();
        assert!(matches!(&err, AssetError::NameEmpty), "got: {err}");
    }

    // R1 — empty reference is rejected
    #[tokio::test]
    async fn test_create_asset_rejects_empty_reference() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .create_asset(CreateAssetDTO {
                reference: "".to_string(),
                isin: None,
                ..base_dto("Bond")
            })
            .await
            .unwrap_err();
        assert!(matches!(&err, AssetError::ReferenceEmpty), "got: {err}");
    }

    // R1 — invalid currency is rejected
    #[tokio::test]
    async fn test_create_asset_rejects_invalid_currency() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .create_asset(CreateAssetDTO {
                currency: "INVALID".to_string(),
                ..base_dto("Bond")
            })
            .await
            .unwrap_err();
        assert!(
            matches!(&err, AssetError::InvalidCurrency { .. }),
            "got: {err}"
        );
    }

    // R1 — risk level out of range is rejected
    #[tokio::test]
    async fn test_create_asset_rejects_invalid_risk_level() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .create_asset(CreateAssetDTO {
                risk_level: 6,
                ..base_dto("Bond")
            })
            .await
            .unwrap_err();
        assert!(
            matches!(&err, AssetError::InvalidRiskLevel { .. }),
            "got: {err}"
        );
    }

    // R4 — service normalizes reference to uppercase before passing to asset_repo.create
    #[tokio::test]
    async fn test_create_asset_normalizes_reference_to_uppercase() {
        let mut ar = MockAssetRepository::new();
        ar.expect_create()
            .withf(|a| a.reference == "AAPL")
            .times(1)
            .return_once(Ok);
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(ar, cr, MockAssetPriceRepository::new());
        let asset = svc
            .create_asset(CreateAssetDTO {
                reference: "aapl".to_string(),
                isin: None,
                ..base_dto("Apple")
            })
            .await
            .unwrap();
        assert_eq!(asset.reference, "AAPL");
    }

    // R4 — service trims reference spaces before passing to asset_repo.create
    #[tokio::test]
    async fn test_create_asset_normalizes_reference_trims_spaces() {
        let mut ar = MockAssetRepository::new();
        ar.expect_create()
            .withf(|a| a.reference == "AAPL")
            .times(1)
            .return_once(Ok);
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(ar, cr, MockAssetPriceRepository::new());
        let asset = svc
            .create_asset(CreateAssetDTO {
                reference: "  AAPL  ".to_string(),
                isin: None,
                ..base_dto("Apple")
            })
            .await
            .unwrap();
        assert_eq!(asset.reference, "AAPL");
    }

    // AST-024 — create_asset passes the interest_bearing opt-in through to the
    // persisted aggregate.
    #[tokio::test]
    async fn test_create_asset_passes_interest_bearing_flag() {
        let mut ar = MockAssetRepository::new();
        ar.expect_create()
            .withf(|a| a.interest_bearing)
            .times(1)
            .return_once(Ok);
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(ar, cr, MockAssetPriceRepository::new());
        let asset = svc
            .create_asset(CreateAssetDTO {
                interest_bearing: true,
                ..base_dto("Euro Fund")
            })
            .await
            .unwrap();
        assert!(asset.interest_bearing);
    }

    // R5/R6 — updating an archived asset is rejected
    #[tokio::test]
    async fn test_update_archived_asset_is_rejected() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-id", true))));
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(ar, cr, MockAssetPriceRepository::new());
        let err = svc
            .update_asset(UpdateAssetDTO {
                asset_id: "asset-id".to_string(),
                name: "Apple Updated".to_string(),
                reference: "AAPL".to_string(),
                isin: None,
                class: AssetClass::Stocks,
                currency: "USD".to_string(),
                risk_level: 4,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap_err();
        assert!(matches!(&err, AssetError::Archived), "got: {err}");
    }

    // R6 — service calls asset_repo.archive with the correct id
    #[tokio::test]
    async fn test_archive_asset_delegates_to_repo() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        ar.expect_archive()
            .withf(|id| id == "asset-id")
            .times(1)
            .return_once(|_| Ok(()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        svc.archive_asset("asset-id").await.unwrap();
    }

    // R18 — service calls asset_repo.unarchive with the correct id
    #[tokio::test]
    async fn test_unarchive_asset_delegates_to_repo() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_asset("asset-id", true))));
        ar.expect_unarchive()
            .withf(|id| id == "asset-id")
            .times(1)
            .return_once(|_| Ok(()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        svc.unarchive_asset("asset-id").await.unwrap();
    }

    // MKT-156 — service calls asset_repo.block_price_refresh with the correct id
    #[tokio::test]
    async fn test_block_price_refresh_delegates_to_repo() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        ar.expect_block_price_refresh()
            .withf(|id| id == "asset-id")
            .times(1)
            .return_once(|_| Ok(()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        svc.block_price_refresh("asset-id").await.unwrap();
    }

    // MKT-156 — service calls asset_repo.unblock_price_refresh with the correct id
    #[tokio::test]
    async fn test_unblock_price_refresh_delegates_to_repo() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        ar.expect_unblock_price_refresh()
            .withf(|id| id == "asset-id")
            .times(1)
            .return_once(|_| Ok(()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        svc.unblock_price_refresh("asset-id").await.unwrap();
    }

    // MKT-154 / CSH-016 — service rejects locking the system Cash Asset; no repo write
    #[tokio::test]
    async fn test_block_price_refresh_rejects_cash_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_cash_asset("cash-usd"))));
        ar.expect_block_price_refresh().times(0);
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc.block_price_refresh("cash-usd").await.unwrap_err();
        assert!(matches!(err, AssetError::CashAssetNotEditable));
    }

    // R7 — get_all_assets delegates to asset_repo.get_all (not get_all_including_archived)
    #[tokio::test]
    async fn test_get_all_assets_excludes_archived() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_all()
            .times(1)
            .return_once(|| Ok(vec![make_asset("active-id", false)]));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let result = svc.get_all_assets().await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "active-id");
    }

    // R19 — get_all_assets_with_archived delegates to asset_repo.get_all_including_archived
    #[tokio::test]
    async fn test_get_all_assets_with_archived_includes_both() {
        let active = make_asset("active-id", false);
        let archived = make_asset("archived-id", true);
        let mut ar = MockAssetRepository::new();
        ar.expect_get_all_including_archived()
            .times(1)
            .return_once(move || Ok(vec![active, archived]));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let all = svc.get_all_assets_with_archived().await.unwrap();
        assert!(all.iter().any(|a| a.id == "active-id"));
        assert!(all.iter().any(|a| a.id == "archived-id"));
    }

    // Category tests

    // R1 — duplicate name, same case: service checks find_by_name before creating
    #[tokio::test]
    async fn test_create_category_rejects_duplicate_same_case() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_find_by_name().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                "existing-id".to_string(),
                "Bonds".to_string(),
            )))
        });
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc.create_category("Bonds").await.unwrap_err();
        assert!(matches!(err, AssetError::DuplicateName), "got: {err:?}");
    }

    // R1 — duplicate name, different case: service checks find_by_name (case-insensitive lookup is the repo's concern)
    #[tokio::test]
    async fn test_create_category_rejects_duplicate_different_case() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_find_by_name().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                "existing-id".to_string(),
                "Bonds".to_string(),
            )))
        });
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc.create_category("bonds").await.unwrap_err();
        assert!(matches!(err, AssetError::DuplicateName), "got: {err:?}");
    }

    // R2 — system category cannot be renamed (check moved into AssetCategory::update_from)
    #[tokio::test]
    async fn test_update_category_rejects_system_category() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                SYSTEM_CATEGORY_ID.to_string(),
                "uncategorized".to_string(),
            )))
        });
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .update_category(SYSTEM_CATEGORY_ID, "Renamed")
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::SystemReadonly), "got: {err:?}");
    }

    // R1 — update with name already taken by a different category
    #[tokio::test]
    async fn test_update_category_rejects_duplicate_name() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                "cat2-id".to_string(),
                "Stocks".to_string(),
            )))
        });
        cr.expect_find_by_name().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                "other-id".to_string(),
                "Bonds".to_string(),
            )))
        });
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc.update_category("cat2-id", "bonds").await.unwrap_err();
        assert!(matches!(err, AssetError::DuplicateName), "got: {err:?}");
    }

    // R2 — system category cannot be deleted (check moved into AssetCategory::ensure_deletable)
    #[tokio::test]
    async fn test_delete_category_rejects_system_category() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                SYSTEM_CATEGORY_ID.to_string(),
                "uncategorized".to_string(),
            )))
        });
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc.delete_category(SYSTEM_CATEGORY_ID).await.unwrap_err();
        assert!(matches!(err, AssetError::SystemProtected), "got: {err:?}");
    }

    // R3 — service calls reassign_assets_and_delete with the category id and system fallback
    #[tokio::test]
    async fn test_delete_category_reassigns_assets_to_default() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                "bonds-id".to_string(),
                "Bonds".to_string(),
            )))
        });
        cr.expect_reassign_assets_and_delete()
            .withf(|cat_id, fallback_id| cat_id == "bonds-id" && fallback_id == SYSTEM_CATEGORY_ID)
            .times(1)
            .return_once(|_, _| Ok(()));
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        svc.delete_category("bonds-id").await.unwrap();
    }

    // MKT-043 — record_asset_price rejects unknown asset
    #[tokio::test]
    async fn test_record_asset_price_rejects_unknown_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id().times(1).return_once(|_| Ok(None));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .record_asset_price("nonexistent-id", "2026-01-01", 100.0)
            .await
            .unwrap_err();
        assert!(
            matches!(
                &err,
                AssetError::AssetNotFound { id }
                    if id == "nonexistent-id"
            ),
            "got: {err:?}"
        );
    }

    // AST-006 — record_asset_price rejects archived asset
    #[tokio::test]
    async fn test_record_asset_price_rejects_archived_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("archived-id", true))));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .record_asset_price("archived-id", "2026-01-01", 100.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::Archived), "got: {err:?}");
    }

    // MKT-021 — record_asset_price rejects price <= 0
    #[tokio::test]
    async fn test_record_asset_price_rejects_non_positive_price() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .record_asset_price("asset-id", "2026-01-01", 0.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::NotPositive), "got: {err:?}");
    }

    // MKT-022 — record_asset_price rejects a future date
    #[tokio::test]
    async fn test_record_asset_price_rejects_future_date() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .record_asset_price("asset-id", "2099-12-31", 100.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::DateInFuture), "got: {err:?}");
    }

    // MKT-025, MKT-026 — record_asset_price calls upsert with correct micros and publishes event
    #[tokio::test]
    async fn test_record_asset_price_upserts_and_publishes_event_on_success() {
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_upsert()
            .withf(|p| p.asset_id == "asset-id" && p.date == "2026-01-01" && p.price == 150_500_000)
            .times(1)
            .return_once(|_| Ok(()));
        let svc = make_svc(ar, MockAssetCategoryRepository::new(), pr).with_event_bus(bus);
        svc.record_asset_price("asset-id", "2026-01-01", 150.5)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_millis(200), rx.changed())
            .await
            .expect("event not received within 200ms")
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::AssetPriceUpdated);
    }

    // MKT-057 — notify_asset_price_updated publishes AssetPriceUpdated when a bus is configured
    #[tokio::test]
    async fn notify_asset_price_updated_publishes_event() {
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        )
        .with_event_bus(bus);
        svc.notify_asset_price_updated();
        tokio::time::timeout(Duration::from_millis(200), rx.changed())
            .await
            .expect("event not received within 200ms")
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::AssetPriceUpdated);
    }

    // MKT-031 — get_latest_price returns None when no price has been recorded
    #[tokio::test]
    async fn get_latest_price_returns_none_when_no_price_recorded() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_latest().times(1).return_once(|_| Ok(None));
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let result = svc.get_latest_price("asset-id").await.unwrap();
        assert!(result.is_none());
    }

    // MKT-031 — get_latest_price delegates to price_repo.get_latest and returns its result
    #[tokio::test]
    async fn get_latest_price_returns_latest_price_when_one_exists() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_latest()
            .times(1)
            .return_once(|_| Ok(Some(make_price("asset-id", "2026-01-03", 120_000_000))));
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let latest = svc.get_latest_price("asset-id").await.unwrap().unwrap();
        assert_eq!(latest.date, "2026-01-03");
        assert_eq!(latest.price, 120_000_000);
    }

    // -------------------------------------------------------------------------
    // get_asset_prices (MKT-072)
    // -------------------------------------------------------------------------

    // MKT-072 — get_asset_prices returns AssetNotFound for a nonexistent asset_id
    #[tokio::test]
    async fn test_get_asset_prices_rejects_unknown_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id().times(1).return_once(|_| Ok(None));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc.get_asset_prices("nonexistent-id").await.unwrap_err();
        assert!(
            matches!(
                &err,
                AssetError::AssetNotFound { id }
                    if id == "nonexistent-id"
            ),
            "got: {err:?}"
        );
    }

    // AST-006 — archive blocks mutations only; reads stay available.
    #[tokio::test]
    async fn test_get_asset_prices_succeeds_for_archived_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("archived-id", true))));
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset()
            .times(1)
            .return_once(|_| Ok(vec![make_price("archived-id", "2026-01-01", 100_000_000)]));
        let svc = make_svc(ar, MockAssetCategoryRepository::new(), pr);
        let prices = svc.get_asset_prices("archived-id").await.unwrap();
        assert_eq!(prices.len(), 1);
    }

    // MKT-072 — get_asset_prices returns an empty list when the asset exists but has no prices
    #[tokio::test]
    async fn test_get_asset_prices_returns_empty_list_when_no_prices() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset()
            .times(1)
            .return_once(|_| Ok(vec![]));
        let svc = make_svc(ar, MockAssetCategoryRepository::new(), pr);
        let prices = svc.get_asset_prices("asset-id").await.unwrap();
        assert!(prices.is_empty());
    }

    // MKT-072 — get_asset_prices passes through whatever order price_repo returns
    #[tokio::test]
    async fn test_get_asset_prices_returns_all_records_sorted_date_descending() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset().times(1).return_once(|_| {
            Ok(vec![
                make_price("asset-id", "2026-01-03", 130_000_000),
                make_price("asset-id", "2026-01-02", 120_000_000),
                make_price("asset-id", "2026-01-01", 100_000_000),
            ])
        });
        let svc = make_svc(ar, MockAssetCategoryRepository::new(), pr);
        let prices = svc.get_asset_prices("asset-id").await.unwrap();
        assert_eq!(prices.len(), 3);
        assert_eq!(prices[0].date, "2026-01-03");
        assert_eq!(prices[1].date, "2026-01-02");
        assert_eq!(prices[2].date, "2026-01-01");
        assert_eq!(prices[0].price, 130_000_000);
        assert_eq!(prices[1].price, 120_000_000);
        assert_eq!(prices[2].price, 100_000_000);
    }

    // MKT-072 — get_asset_prices calls price_repo with the requested asset_id
    #[tokio::test]
    async fn test_get_asset_prices_scoped_to_requested_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .withf(|id| id == "asset-a-id")
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-a-id", false))));
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset()
            .withf(|id| id == "asset-a-id")
            .times(1)
            .return_once(|_| Ok(vec![make_price("asset-a-id", "2026-01-01", 100_000_000)]));
        let svc = make_svc(ar, MockAssetCategoryRepository::new(), pr);
        let prices = svc.get_asset_prices("asset-a-id").await.unwrap();
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].asset_id, "asset-a-id");
    }

    // -------------------------------------------------------------------------
    // update_asset_price (MKT-082, MKT-083, MKT-084, MKT-085)
    // -------------------------------------------------------------------------

    // MKT-082 — validation runs before any repo call; no mock expectations needed
    #[tokio::test]
    async fn test_update_asset_price_rejects_non_positive_price() {
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .update_asset_price("asset-id", "2026-01-01", "2026-01-01", 0.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::NotPositive), "got: {err:?}");
    }

    // MKT-082 — non-finite check runs before micro conversion; no repo calls
    #[tokio::test]
    async fn test_update_asset_price_rejects_non_finite_price() {
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .update_asset_price("asset-id", "2026-01-01", "2026-01-01", f64::NAN)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::NonFinite), "got: {err:?}");
    }

    // MKT-082 — future new_date rejected by AssetPrice::new before DB lookup
    #[tokio::test]
    async fn test_update_asset_price_rejects_future_date() {
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .update_asset_price("asset-id", "2026-01-01", "2099-12-31", 150.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::DateInFuture), "got: {err:?}");
    }

    // AST-006 — update_asset_price rejects archived asset
    #[tokio::test]
    async fn test_update_asset_price_rejects_archived_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("archived-id", true))));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .update_asset_price("archived-id", "2026-01-01", "2026-01-01", 100.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::Archived), "got: {err:?}");
    }

    // MKT-083 — returns NotFound when get_by_asset_and_date returns None
    #[tokio::test]
    async fn test_update_asset_price_returns_not_found_for_missing_record() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(None));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let err = svc
            .update_asset_price("asset-id", "2026-01-01", "2026-01-01", 100.0)
            .await
            .unwrap_err();
        assert!(
            matches!(
                &err,
                AssetError::PriceNotFound { asset_id, date }
                    if asset_id == "asset-id" && date == "2026-01-01"
            ),
            "got: {err:?}"
        );
    }

    // MKT-083 — same original_date and new_date: service calls upsert (not replace_atomic)
    #[tokio::test]
    async fn test_update_asset_price_same_date_updates_price_in_place() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(Some(make_price("asset-id", "2026-01-01", 100_000_000))));
        pr.expect_upsert()
            .withf(|p| p.date == "2026-01-01" && p.price == 150_000_000)
            .times(1)
            .return_once(|_| Ok(()));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        );
        svc.update_asset_price("asset-id", "2026-01-01", "2026-01-01", 150.0)
            .await
            .unwrap();
    }

    // MKT-084 — different dates: service calls replace_atomic with original_date and new price
    #[tokio::test]
    async fn test_update_asset_price_date_change_deletes_old_and_upserts_new() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(Some(make_price("asset-id", "2026-01-01", 100_000_000))));
        pr.expect_replace_atomic()
            .withf(|id, orig, new_p| {
                id == "asset-id"
                    && orig == "2026-01-01"
                    && new_p.date == "2026-01-02"
                    && new_p.price == 110_000_000
            })
            .times(1)
            .return_once(|_, _, _| Ok(()));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        );
        svc.update_asset_price("asset-id", "2026-01-01", "2026-01-02", 110.0)
            .await
            .unwrap();
    }

    // MKT-084 — date change always calls replace_atomic regardless of whether target date exists
    #[tokio::test]
    async fn test_update_asset_price_date_change_overwrites_existing_target_date() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(Some(make_price("asset-id", "2026-01-01", 100_000_000))));
        pr.expect_replace_atomic()
            .withf(|id, orig, new_p| {
                id == "asset-id"
                    && orig == "2026-01-01"
                    && new_p.date == "2026-01-02"
                    && new_p.price == 200_000_000
            })
            .times(1)
            .return_once(|_, _, _| Ok(()));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        );
        svc.update_asset_price("asset-id", "2026-01-01", "2026-01-02", 200.0)
            .await
            .unwrap();
    }

    // MKT-085 — publishes AssetPriceUpdated after a successful update
    #[tokio::test]
    async fn test_update_asset_price_publishes_asset_price_updated_event() {
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(Some(make_price("asset-id", "2026-01-01", 100_000_000))));
        pr.expect_upsert().times(1).return_once(|_| Ok(()));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        )
        .with_event_bus(bus);
        svc.update_asset_price("asset-id", "2026-01-01", "2026-01-01", 150.0)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_millis(200), rx.changed())
            .await
            .expect("event not received within 200ms")
            .unwrap();
        assert_eq!(*rx.borrow(), Event::AssetPriceUpdated);
    }

    // -------------------------------------------------------------------------
    // delete_asset_price (MKT-090, MKT-091)
    // -------------------------------------------------------------------------

    // AST-006 — delete_asset_price rejects archived asset
    #[tokio::test]
    async fn test_delete_asset_price_rejects_archived_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("archived-id", true))));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .delete_asset_price("archived-id", "2026-01-01")
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::Archived), "got: {err:?}");
    }

    // MKT-090 — returns NotFound when get_by_asset_and_date returns None
    #[tokio::test]
    async fn delete_asset_price_returns_not_found_for_missing_record() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(None));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let err = svc
            .delete_asset_price("asset-id", "2026-01-01")
            .await
            .unwrap_err();
        assert!(
            matches!(
                &err,
                AssetError::PriceNotFound { asset_id, date }
                    if asset_id == "asset-id" && date == "2026-01-01"
            ),
            "got: {err:?}"
        );
    }

    // MKT-090 — calls price_repo.delete with the correct (asset_id, date) after existence check
    #[tokio::test]
    async fn delete_asset_price_removes_the_record() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(Some(make_price("asset-id", "2026-01-01", 100_000_000))));
        pr.expect_delete()
            .withf(|id, date| id == "asset-id" && date == "2026-01-01")
            .times(1)
            .return_once(|_, _| Ok(()));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        );
        svc.delete_asset_price("asset-id", "2026-01-01")
            .await
            .unwrap();
    }

    // MKT-091 — publishes AssetPriceUpdated after a successful delete
    #[tokio::test]
    async fn delete_asset_price_publishes_asset_price_updated_event() {
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(Some(make_price("asset-id", "2026-01-01", 100_000_000))));
        pr.expect_delete().times(1).return_once(|_, _| Ok(()));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        )
        .with_event_bus(bus);
        svc.delete_asset_price("asset-id", "2026-01-01")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_millis(200), rx.changed())
            .await
            .expect("event not received within 200ms")
            .unwrap();
        assert_eq!(*rx.borrow(), Event::AssetPriceUpdated);
    }

    // -------------------------------------------------------------------------
    // Asset price — infra-translation coverage (gold rule)
    // -------------------------------------------------------------------------

    fn db_err() -> anyhow::Error {
        anyhow::anyhow!("simulated database failure")
    }

    // record_asset_price translates raw asset_repo failure → DatabaseError on the asset leaf
    #[tokio::test]
    async fn record_asset_price_translates_asset_repo_failure_to_database_error() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Err(db_err()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .record_asset_price("asset-id", "2026-01-01", 100.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // record_asset_price translates raw price_repo upsert failure → DatabaseError on the price leaf
    #[tokio::test]
    async fn record_asset_price_translates_price_repo_failure_to_database_error() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_upsert().times(1).return_once(|_| Err(db_err()));
        let svc = make_svc(ar, MockAssetCategoryRepository::new(), pr);
        let err = svc
            .record_asset_price("asset-id", "2026-01-01", 150.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // get_asset_prices translates raw price_repo get_all failure → DatabaseError
    #[tokio::test]
    async fn get_asset_prices_translates_price_repo_failure_to_database_error() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .times(1)
            .return_once(|_| Ok(Some(make_asset("asset-id", false))));
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset()
            .times(1)
            .return_once(|_| Err(db_err()));
        let svc = make_svc(ar, MockAssetCategoryRepository::new(), pr);
        let err = svc.get_asset_prices("asset-id").await.unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // update_asset_price (different dates) translates raw replace_atomic failure → DatabaseError
    #[tokio::test]
    async fn update_asset_price_translates_replace_atomic_failure_to_database_error() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(Some(make_price("asset-id", "2026-01-01", 100_000_000))));
        pr.expect_replace_atomic()
            .times(1)
            .return_once(|_, _, _| Err(db_err()));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let err = svc
            .update_asset_price("asset-id", "2026-01-01", "2026-01-02", 110.0)
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // delete_asset_price translates raw price_repo delete failure → DatabaseError
    #[tokio::test]
    async fn delete_asset_price_translates_repo_failure_to_database_error() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_by_asset_and_date()
            .times(1)
            .return_once(|_, _| Ok(Some(make_price("asset-id", "2026-01-01", 100_000_000))));
        pr.expect_delete()
            .times(1)
            .return_once(|_, _| Err(db_err()));
        let svc = make_svc(
            ar_with_writable_asset("asset-id"),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let err = svc
            .delete_asset_price("asset-id", "2026-01-01")
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // update_asset_price surfaces InvalidDateFormat for a malformed new_date,
    // echoing the offending input on the wire (boyscout: previously surfaced as opaque Unknown).
    #[tokio::test]
    async fn update_asset_price_surfaces_invalid_date_format() {
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .update_asset_price("asset-id", "2026-01-01", "not-a-date", 100.0)
            .await
            .unwrap_err();
        assert!(
            matches!(
                &err,
                AssetError::InvalidDateFormat { date }
                    if date == "not-a-date"
            ),
            "got: {err:?}"
        );
    }

    // ── Mock-based unit tests for event bus branches and error paths ──────────

    #[tokio::test]
    async fn update_asset_returns_archived_error() {
        let archived_asset = make_asset("a-id", true);
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(move |_| Ok(Some(archived_asset)));
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(ar, cr, MockAssetPriceRepository::new());

        let err = svc
            .update_asset(UpdateAssetDTO {
                asset_id: "a-id".to_string(),
                name: "New".to_string(),
                reference: "REF".to_string(),
                isin: None,
                class: AssetClass::Cash,
                currency: "USD".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap_err();

        assert!(
            matches!(&err, AssetError::Archived),
            "expected Archived, got: {err}"
        );
    }

    #[tokio::test]
    async fn update_asset_returns_category_not_found() {
        let active_asset = make_asset("a-id", false);
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(move |_| Ok(Some(active_asset)));
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().return_once(|_| Ok(None));
        let svc = make_svc(ar, cr, MockAssetPriceRepository::new());

        let err = svc
            .update_asset(UpdateAssetDTO {
                asset_id: "a-id".to_string(),
                name: "New".to_string(),
                reference: "REF".to_string(),
                isin: None,
                class: AssetClass::Cash,
                currency: "USD".to_string(),
                risk_level: 1,
                category_id: "missing-cat".to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap_err();

        assert!(
            matches!(&err, AssetError::CategoryNotFound { .. }),
            "expected AssetError::CategoryNotFound, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_archive_asset_emits_event_when_bus_present() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_asset("a-id", false))));
        ar.expect_archive().times(1).return_once(|_| Ok(()));
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        )
        .with_event_bus(Arc::clone(&bus));

        svc.archive_asset("a-id").await.unwrap();

        rx.changed()
            .await
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::AssetUpdated);
    }

    #[tokio::test]
    async fn test_unarchive_asset_emits_event_when_bus_present() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_asset("a-id", true))));
        ar.expect_unarchive().times(1).return_once(|_| Ok(()));
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        )
        .with_event_bus(Arc::clone(&bus));

        svc.unarchive_asset("a-id").await.unwrap();

        rx.changed()
            .await
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::AssetUpdated);
    }

    #[tokio::test]
    async fn test_delete_asset_emits_event_when_bus_present() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_asset("a-id", false))));
        ar.expect_delete().times(1).return_once(|_| Ok(()));
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        )
        .with_event_bus(Arc::clone(&bus));

        svc.delete_asset("a-id").await.unwrap();

        rx.changed()
            .await
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::AssetUpdated);
    }

    // CSH-016 — update_asset rejects a system Cash Asset
    #[tokio::test]
    async fn test_update_asset_rejects_cash_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_cash_asset("system-cash-USD"))));
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .return_once(|_| Ok(Some(make_category())));
        let svc = make_svc(ar, cr, MockAssetPriceRepository::new());

        let err = svc
            .update_asset(UpdateAssetDTO {
                asset_id: "system-cash-USD".to_string(),
                name: "Renamed".to_string(),
                reference: "USD".to_string(),
                isin: None,
                class: AssetClass::Cash,
                currency: "USD".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap_err();
        assert!(
            matches!(&err, AssetError::CashAssetNotEditable),
            "got: {err}"
        );
    }

    // CSH-016 — archive_asset rejects a system Cash Asset
    #[tokio::test]
    async fn test_archive_asset_rejects_cash_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_cash_asset("system-cash-USD"))));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );

        let err = svc.archive_asset("system-cash-USD").await.unwrap_err();
        assert!(
            matches!(&err, AssetError::CashAssetNotEditable),
            "got: {err}"
        );
    }

    // CSH-016 — unarchive_asset rejects a system Cash Asset
    #[tokio::test]
    async fn test_unarchive_asset_rejects_cash_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_cash_asset("system-cash-USD"))));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );

        let err = svc.unarchive_asset("system-cash-USD").await.unwrap_err();
        assert!(
            matches!(&err, AssetError::CashAssetNotEditable),
            "got: {err}"
        );
    }

    // CSH-016 — delete_asset rejects a system Cash Asset
    #[tokio::test]
    async fn test_delete_asset_rejects_cash_asset() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Ok(Some(make_cash_asset("system-cash-USD"))));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );

        let err = svc.delete_asset("system-cash-USD").await.unwrap_err();
        assert!(
            matches!(&err, AssetError::CashAssetNotEditable),
            "got: {err}"
        );
    }

    // CSH-016 — archive_asset surfaces NotFound when the id is unknown
    #[tokio::test]
    async fn test_archive_asset_returns_not_found_for_unknown_id() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id().return_once(|_| Ok(None));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );

        let err = svc.archive_asset("missing").await.unwrap_err();
        assert!(
            matches!(&err, AssetError::AssetNotFound { .. }),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn test_create_category_emits_event_when_bus_present() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_find_by_name().times(1).return_once(|_| Ok(None));
        cr.expect_create().times(1).return_once(Ok);
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        )
        .with_event_bus(Arc::clone(&bus));

        svc.create_category("NewCat").await.unwrap();

        rx.changed()
            .await
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::CategoryUpdated);
    }

    #[tokio::test]
    async fn test_update_category_emits_event_when_bus_present() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                "some-id".to_string(),
                "Old".to_string(),
            )))
        });
        cr.expect_find_by_name().times(1).return_once(|_| Ok(None));
        cr.expect_update().times(1).return_once(Ok);
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        )
        .with_event_bus(Arc::clone(&bus));

        svc.update_category("some-id", "Updated").await.unwrap();

        rx.changed()
            .await
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::CategoryUpdated);
    }

    #[tokio::test]
    async fn test_delete_category_emits_event_when_bus_present() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                "cat-id".to_string(),
                "Bonds".to_string(),
            )))
        });
        cr.expect_reassign_assets_and_delete()
            .times(1)
            .return_once(|_, _| Ok(()));
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        )
        .with_event_bus(Arc::clone(&bus));

        svc.delete_category("some-cat-id").await.unwrap();

        rx.changed()
            .await
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::CategoryUpdated);
    }

    // -------------------------------------------------------------------------
    // Category CRUD typed-error coverage (PR 6 — first PR enforcing the new
    // gold infra-translation rule: per-BC `*ApplicationError::DatabaseError`
    // with no `hint` payload; full diagnostic preserved server-side via
    // `tracing::error!` only.)
    // -------------------------------------------------------------------------

    #[derive(Debug, thiserror::Error)]
    #[error("simulated DB failure")]
    struct SimulatedDbError;

    // PR 6 — create_category surfaces find_by_name repo failure as
    // AssetError::DatabaseError (no payload — diagnostic stays
    // in tracing). Exercises the typed-error contract for the uniqueness
    // pre-check failure path.
    #[tokio::test]
    async fn test_create_category_returns_database_error_when_find_by_name_fails() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_find_by_name()
            .times(1)
            .return_once(|_| Err(SimulatedDbError.into()));
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc.create_category("anything").await.unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // PR 6 — update_category surfaces get_by_id Ok(None) as a typed
    // application-class NotFound carrying the requested ID. Distinct from the
    // get_by_id Err path (which becomes DatabaseError).
    #[tokio::test]
    async fn test_update_category_returns_not_found_when_aggregate_missing() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().times(1).return_once(|_| Ok(None));
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc
            .update_category("missing-id", "Anything")
            .await
            .unwrap_err();
        assert!(
            matches!(
                &err,
                AssetError::CategoryNotFound { id }
                    if id == "missing-id"
            ),
            "got: {err:?}"
        );
    }

    // PR 6 — delete_category surfaces reassign_assets_and_delete repo failure
    // as DatabaseError. Distinct from the get_by_id Ok(None) path (NotFound)
    // and the ensure_deletable path (Validation::SystemProtected).
    #[tokio::test]
    async fn test_delete_category_returns_database_error_when_reassign_fails() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().times(1).return_once(|_| {
            Ok(Some(AssetCategory::from_storage(
                "some-id".to_string(),
                "Bonds".to_string(),
            )))
        });
        cr.expect_reassign_assets_and_delete()
            .times(1)
            .return_once(|_, _| Err(SimulatedDbError.into()));
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );
        let err = svc.delete_category("some-id").await.unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // Asset CRUD typed-error coverage (PR 7)
    // -------------------------------------------------------------------------

    // create_asset surfaces asset_repo.create failure as DatabaseError after
    // category lookup succeeds. Distinct path from the category-side
    // DatabaseError covered above.
    #[tokio::test]
    async fn test_create_asset_returns_database_error_when_repo_create_fails() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id()
            .return_once(|_| Ok(Some(make_category())));
        let mut ar = MockAssetRepository::new();
        ar.expect_create()
            .return_once(|_| Err(SimulatedDbError.into()));
        let svc = make_svc(ar, cr, MockAssetPriceRepository::new());
        let err = svc.create_asset(base_dto("Bond")).await.unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // load_asset_for_crud surfaces Ok(None) as typed NotFound carrying the ID.
    // Exercised through archive_asset (the simplest write that hits the helper).
    #[tokio::test]
    async fn test_archive_asset_returns_not_found_with_id() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id().return_once(|_| Ok(None));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc.archive_asset("missing-id").await.unwrap_err();
        assert!(
            matches!(
                &err,
                AssetError::AssetNotFound { id }
                    if id == "missing-id"
            ),
            "got: {err:?}"
        );
    }

    // delete_asset surfaces asset_repo.delete failure as DatabaseError after
    // the ensure_user_managed invariant passes.
    #[tokio::test]
    async fn test_delete_asset_returns_database_error_when_repo_delete_fails() {
        let asset = make_asset("some-id", false);
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id().return_once(move |_| Ok(Some(asset)));
        ar.expect_delete()
            .return_once(|_| Err(SimulatedDbError.into()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc.delete_asset("some-id").await.unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // load_asset_for_crud surfaces a get_by_id repo error (distinct from
    // Ok(None) → NotFound) as DatabaseError. Exercised through archive_asset
    // (the simplest write that hits the helper).
    #[tokio::test]
    async fn test_archive_asset_returns_database_error_when_load_fails() {
        let mut ar = MockAssetRepository::new();
        ar.expect_get_by_id()
            .return_once(|_| Err(SimulatedDbError.into()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let err = svc.archive_asset("some-id").await.unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // record_daily_closes (SPF-030/031/032/034)
    // -------------------------------------------------------------------------

    // SPF-034 — each DatedClose is upserted with source=YahooFinance; the
    // returned count matches the number of entries written.
    #[tokio::test]
    async fn record_daily_closes_upserts_each_entry_with_yahoo_finance_source() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_upsert()
            .withf(|p| {
                p.asset_id == "asset-id"
                    && p.source == AssetPriceSource::YahooFinance
                    && (p.date == "2026-06-08" || p.date == "2026-06-09")
            })
            .times(2)
            .returning(|_| Ok(()));
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let count = svc
            .record_daily_closes(
                "asset-id",
                vec![
                    DatedClose {
                        date: "2026-06-08".to_string(),
                        price: 290_000_000,
                    },
                    DatedClose {
                        date: "2026-06-09".to_string(),
                        price: 291_000_000,
                    },
                ],
            )
            .await
            .unwrap();
        assert_eq!(count, 2, "SPF-050 relies on this count for updated_count");
    }

    // SPF-032 — an empty series (all days absent — non-trading days) writes
    // nothing and returns Ok(0); this is not a skip or an error.
    #[tokio::test]
    async fn record_daily_closes_with_empty_series_writes_nothing() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_upsert().times(0);
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let count = svc.record_daily_closes("asset-id", vec![]).await.unwrap();
        assert_eq!(count, 0);
    }

    // A per-write repository failure surfaces as the typed DatabaseError.
    #[tokio::test]
    async fn record_daily_closes_surfaces_database_error_on_upsert_failure() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_upsert()
            .return_once(|_| Err(SimulatedDbError.into()));
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        );
        let err = svc
            .record_daily_closes(
                "asset-id",
                vec![DatedClose {
                    date: "2026-06-08".to_string(),
                    price: 290_000_000,
                }],
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    fn backfill_close(date: &str) -> DatedClose {
        DatedClose {
            date: date.to_string(),
            price: 100_000_000,
        }
    }

    // MKT-192/194 — only the dates with no recorded price are written, whatever the
    // recorded price's source; the others are counted as skipped.
    #[tokio::test]
    async fn record_missing_daily_closes_writes_only_the_dates_without_a_price() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset().times(1).return_once(|_| {
            Ok(vec![AssetPrice::restore(
                "asset-id".to_string(),
                "2026-06-09".to_string(),
                50_000_000,
                AssetPriceSource::Manual,
            )])
        });
        pr.expect_upsert()
            .withf(|p| {
                p.asset_id == "asset-id"
                    && p.source == AssetPriceSource::YahooFinance
                    && (p.date == "2026-06-08" || p.date == "2026-06-10")
            })
            .times(2)
            .returning(|_| Ok(()));
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        );

        let fill = svc
            .record_missing_daily_closes(
                "asset-id",
                vec![
                    backfill_close("2026-06-08"),
                    backfill_close("2026-06-09"),
                    backfill_close("2026-06-10"),
                ],
            )
            .await
            .unwrap();

        assert_eq!(
            fill,
            PriceHistoryBackfillOutcome {
                written: 2,
                already_priced: 1
            }
        );
    }

    // MKT-194 — a backfill that wrote at least one close publishes AssetPriceUpdated.
    #[tokio::test]
    async fn record_missing_daily_closes_publishes_when_a_close_was_written() {
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset().return_once(|_| Ok(vec![]));
        pr.expect_upsert().times(1).returning(|_| Ok(()));
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        )
        .with_event_bus(bus);

        svc.record_missing_daily_closes("asset-id", vec![backfill_close("2026-06-08")])
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_millis(200), rx.changed())
            .await
            .expect("event not received within 200ms")
            .expect("watch sender dropped before event fired");
        assert_eq!(*rx.borrow(), Event::AssetPriceUpdated);
    }

    // MKT-194 — a backfill that wrote nothing publishes nothing. The test above
    // proves the same bus wiring does deliver the event when a close is written.
    #[tokio::test]
    async fn record_missing_daily_closes_publishes_nothing_when_every_date_had_a_price() {
        let bus = Arc::new(SideEffectEventBus::new());
        let mut rx = bus.subscribe();
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset().return_once(|_| {
            Ok(vec![AssetPrice::restore(
                "asset-id".to_string(),
                "2026-06-08".to_string(),
                50_000_000,
                AssetPriceSource::YahooFinance,
            )])
        });
        pr.expect_upsert().times(0);
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        )
        .with_event_bus(bus);

        let fill = svc
            .record_missing_daily_closes("asset-id", vec![backfill_close("2026-06-08")])
            .await
            .unwrap();

        assert_eq!(
            fill,
            PriceHistoryBackfillOutcome {
                written: 0,
                already_priced: 1
            }
        );
        assert!(
            tokio::time::timeout(Duration::from_millis(50), rx.changed())
                .await
                .is_err(),
            "no event may be published when nothing was written"
        );
    }

    // A price listing failure surfaces as DatabaseError before any write.
    #[tokio::test]
    async fn record_missing_daily_closes_surfaces_a_listing_failure() {
        let mut pr = MockAssetPriceRepository::new();
        pr.expect_get_all_for_asset()
            .return_once(|_| Err(SimulatedDbError.into()));
        pr.expect_upsert().times(0);
        let svc = make_svc(
            MockAssetRepository::new(),
            MockAssetCategoryRepository::new(),
            pr,
        );

        let err = svc
            .record_missing_daily_closes("asset-id", vec![backfill_close("2026-06-08")])
            .await
            .unwrap_err();

        assert!(matches!(err, AssetError::DatabaseError), "got: {err:?}");
    }

    // -------------------------------------------------------------------------
    // CFR-017 — apply entry points bypass entry guards
    // -------------------------------------------------------------------------

    use crate::shared::domain::{LogicalTimestamp, Origin, Rank, RecordKind};

    fn incoming_rank(device_id: &str, timestamp: u64) -> Rank {
        Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(timestamp),
            device_id: device_id.to_string(),
        }
    }

    // CFR-017 — apply_asset writes an archived incoming asset verbatim; no entry guard
    // (e.g. "cannot edit an archived asset") ever runs.
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

    // CFR-017 — apply_asset writes the incoming content verbatim, archived flag included,
    // through the repository's apply write: no create/update guard runs.
    #[tokio::test]
    async fn apply_asset_writes_an_archived_asset_verbatim() {
        let mut ar = MockAssetRepository::new();
        ar.expect_apply_asset()
            .withf(|_, asset, rank| {
                asset.id == "asset-from-laptop" && asset.is_archived && rank.device_id == "laptop"
            })
            .times(1)
            .returning(|_, _, _| Ok(()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let content = serde_json::to_string(&make_asset("asset-from-laptop", true)).unwrap();
        svc.apply_asset(
            &mut *apply_conn().await,
            &content,
            incoming_rank("laptop", 100),
        )
        .await
        .expect("CFR-017: applying an archived asset must succeed, no re-validation");
    }

    // CFR-017 — apply_category writes the incoming category verbatim.
    #[tokio::test]
    async fn apply_category_writes_incoming_category_verbatim() {
        let mut ar = MockAssetRepository::new();
        ar.expect_apply_category()
            .withf(|_, category, _| category.id == "cat-from-laptop" && category.name == "Tech")
            .times(1)
            .returning(|_, _, _| Ok(()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let content = r#"{"id":"cat-from-laptop","name":"Tech"}"#;
        svc.apply_category(
            &mut *apply_conn().await,
            content,
            incoming_rank("laptop", 100),
        )
        .await
        .expect("CFR-017: applying an incoming category must succeed");
    }

    // CFR-050 — apply_asset_price writes the observation the engine decided prevails; the
    // service itself runs no rank check.
    #[tokio::test]
    async fn apply_asset_price_merges_by_observation_rule() {
        let mut ar = MockAssetRepository::new();
        ar.expect_apply_asset_price()
            .withf(|_, price, _| price.asset_id == "asset-1" && price.price == 58_250_000)
            .times(1)
            .returning(|_, _, _| Ok(()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        let content =
            r#"{"asset_id":"asset-1","date":"2026-08-21","price":58250000,"source":"Manual"}"#;
        svc.apply_asset_price(
            &mut *apply_conn().await,
            content,
            incoming_rank("laptop", 1_300),
        )
        .await
        .expect("CFR-050: an observation always applies by timestamp, no rank check");
    }

    // SYN-064 — applying an asset, a category, a price or a removal publishes nothing: the
    // sync run announces everything it applied with one SyncCompleted.
    #[tokio::test]
    async fn applied_writes_publish_no_event() {
        let mut ar = MockAssetRepository::new();
        ar.expect_apply_asset_price()
            .times(1)
            .returning(|_, _, _| Ok(()));
        ar.expect_remove_synced()
            .times(3)
            .returning(|_, _, _| Ok(()));
        let bus = Arc::new(SideEffectEventBus::new());
        let rx = bus.subscribe();
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        )
        .with_event_bus(Arc::clone(&bus));

        svc.apply_asset_price(
            &mut *apply_conn().await,
            r#"{"asset_id":"asset-1","date":"2026-08-21","price":58250000,"source":"Manual"}"#,
            incoming_rank("laptop", 1_300),
        )
        .await
        .expect("CFR-050: an incoming price must apply");
        for (kind, identity) in [
            (RecordKind::Asset, "asset-1"),
            (RecordKind::Category, "cat-1"),
            (RecordKind::AssetPrice, "asset-1|2026-08-21"),
        ] {
            svc.apply_removal(&mut *apply_conn().await, kind, identity)
                .await
                .expect("CFR-017: an incoming removal must apply");
        }

        assert!(
            !rx.has_changed().unwrap(),
            "SYN-064: an applied write must not raise its own event"
        );
    }

    // CFR-017 — apply_removal of a category removes it through the repository's apply write.
    #[tokio::test]
    async fn apply_removal_of_a_category_succeeds() {
        let mut ar = MockAssetRepository::new();
        ar.expect_remove_synced()
            .withf(|_, kind, identity| *kind == RecordKind::Category && identity == "cat-1")
            .times(1)
            .returning(|_, _, _| Ok(()));
        let svc = make_svc(
            ar,
            MockAssetCategoryRepository::new(),
            MockAssetPriceRepository::new(),
        );
        svc.apply_removal(&mut *apply_conn().await, RecordKind::Category, "cat-1")
            .await
            .expect("CFR-017: an incoming category removal must apply");
    }

    #[tokio::test]
    async fn update_category_accepts_the_unchanged_label_even_when_it_already_clashes() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().returning(|_| {
            Ok(Some(AssetCategory::from_storage(
                "cat-1".to_string(),
                "Tech".to_string(),
            )))
        });
        cr.expect_find_by_name().returning(|_| {
            Ok(Some(AssetCategory::from_storage(
                "cat-clash".to_string(),
                "Tech".to_string(),
            )))
        });
        cr.expect_update().returning(Ok);
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );

        let result = svc.update_category("cat-1", "Tech").await;
        assert!(
            result.is_ok(),
            "CFR-035: the unchanged label must be accepted despite the clash, got {result:?}"
        );
    }

    // CFR-035 — changing the label TO one another category already holds is still
    // rejected: the guard binds the label being set.
    #[tokio::test]
    async fn update_category_still_rejects_changing_the_label_to_one_another_category_holds() {
        let mut cr = MockAssetCategoryRepository::new();
        cr.expect_get_by_id().returning(|_| {
            Ok(Some(AssetCategory::from_storage(
                "cat-1".to_string(),
                "Old Label".to_string(),
            )))
        });
        cr.expect_find_by_name().returning(|_| {
            Ok(Some(AssetCategory::from_storage(
                "cat-2".to_string(),
                "Growth".to_string(),
            )))
        });
        let svc = make_svc(
            MockAssetRepository::new(),
            cr,
            MockAssetPriceRepository::new(),
        );

        let result = svc.update_category("cat-1", "Growth").await;
        assert!(matches!(result, Err(AssetError::DuplicateName)));
    }
}
