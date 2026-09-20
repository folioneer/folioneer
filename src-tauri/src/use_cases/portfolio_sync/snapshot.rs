//! `ServicePortfolioSnapshot` — the `PortfolioSnapshot` port over the account, asset, and
//! currency services (ADR-004): every synced record this installation holds (SYN-013/021),
//! system-seeded records excluded (SYN-027), serialized exactly as the repositories' change
//! capture serializes them.

use std::sync::Arc;

use crate::context::account::{AccountService, FeeSchedule};
use crate::context::asset::{AssetService, SYSTEM_CATEGORY_ID};
use crate::context::currency::{CurrencyPair, CurrencyService};
use crate::context::sync::{PortfolioRecord, PortfolioSnapshot, SyncError};
use crate::core::cash::{is_cash_asset, SYSTEM_CASH_CATEGORY_ID};
use crate::shared::domain::{RecordIdentity, RecordKind};

/// Reads the whole portfolio through the owning bounded contexts' services.
pub struct ServicePortfolioSnapshot {
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    currency_service: Arc<CurrencyService>,
}

fn record<T: serde::Serialize>(
    record_kind: RecordKind,
    key: &[&str],
    value: &T,
) -> Result<PortfolioRecord, SyncError> {
    Ok(PortfolioRecord {
        record_kind,
        record_identity: RecordIdentity::canonical(record_kind, key),
        content: serde_json::to_string(value)
            .map_err(|error| SyncError::database("snapshot: serialization failed", error))?,
    })
}

/// A fee schedule's content carries its own state only: `last_applied_period` is the derived
/// read of the separately synced catch-up position (CFR-044).
fn fee_schedule_record(schedule: &FeeSchedule) -> Result<PortfolioRecord, SyncError> {
    let mut value = serde_json::to_value(schedule)
        .map_err(|error| SyncError::database("snapshot: serialization failed", error))?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("last_applied_period");
    }
    record(
        RecordKind::FeeSchedule,
        &[&schedule.account_id, &schedule.asset_id],
        &value,
    )
}

impl ServicePortfolioSnapshot {
    /// Creates the snapshot over the three services that own synced records.
    pub fn new(
        account_service: Arc<AccountService>,
        asset_service: Arc<AssetService>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            currency_service,
        }
    }
}

#[async_trait::async_trait]
impl PortfolioSnapshot for ServicePortfolioSnapshot {
    async fn records(&self) -> Result<Vec<PortfolioRecord>, SyncError> {
        let mut records = Vec::new();

        let categories = self
            .asset_service
            .get_all_categories()
            .await
            .map_err(|error| SyncError::database("snapshot: categories", error))?;
        for category in categories.iter().filter(|category| {
            category.id != SYSTEM_CATEGORY_ID && category.id != SYSTEM_CASH_CATEGORY_ID
        }) {
            records.push(record(RecordKind::Category, &[&category.id], category)?);
        }

        let assets = self
            .asset_service
            .get_all_assets_with_archived()
            .await
            .map_err(|error| SyncError::database("snapshot: assets", error))?;
        for asset in assets.iter().filter(|asset| !is_cash_asset(&asset.id)) {
            records.push(record(RecordKind::Asset, &[&asset.id], asset)?);
        }

        let accounts = self
            .account_service
            .get_all()
            .await
            .map_err(|error| SyncError::database("snapshot: accounts", error))?;
        for account in &accounts {
            records.push(record(RecordKind::Account, &[&account.id], account)?);
        }
        for account in &accounts {
            let transactions = self
                .account_service
                .get_all_transactions_for_account(&account.id)
                .await
                .map_err(|error| SyncError::database("snapshot: transactions", error))?;
            for transaction in &transactions {
                records.push(record(
                    RecordKind::Transaction,
                    &[&transaction.id],
                    transaction,
                )?);
            }
            let schedules = self
                .account_service
                .list_fee_schedules_for_account(&account.id)
                .await
                .map_err(|error| SyncError::database("snapshot: fee schedules", error))?;
            for schedule in &schedules {
                records.push(fee_schedule_record(schedule)?);
            }
            let positions = self
                .account_service
                .list_fee_catch_up_positions_for_account(&account.id)
                .await
                .map_err(|error| SyncError::database("snapshot: fee catch-up positions", error))?;
            for position in &positions {
                records.push(record(
                    RecordKind::FeeCatchUpPosition,
                    &[&position.account_id, &position.asset_id],
                    position,
                )?);
            }
            let notes = self
                .account_service
                .get_holding_notes(&account.id)
                .await
                .map_err(|error| SyncError::database("snapshot: holding notes", error))?;
            for note in &notes {
                records.push(record(
                    RecordKind::HoldingNote,
                    &[&note.account_id, &note.asset_id],
                    note,
                )?);
            }
        }

        for asset in assets.iter().filter(|asset| !is_cash_asset(&asset.id)) {
            let prices = self
                .asset_service
                .get_asset_prices(&asset.id)
                .await
                .map_err(|error| SyncError::database("snapshot: asset prices", error))?;
            for price in &prices {
                records.push(record(
                    RecordKind::AssetPrice,
                    &[&price.asset_id, &price.date],
                    price,
                )?);
            }
        }

        let pairs = self
            .currency_service
            .list_currency_pairs()
            .await
            .map_err(|error| SyncError::database("snapshot: currency pairs", error))?;
        for summary in &pairs {
            let pair = CurrencyPair::from_storage(
                summary.from_currency.clone(),
                summary.to_currency.clone(),
            );
            records.push(record(
                RecordKind::CurrencyPair,
                &[&pair.from_currency, &pair.to_currency],
                &pair,
            )?);
        }
        for summary in &pairs {
            let rates = self
                .currency_service
                .list_currency_rates(summary.from_currency.clone(), summary.to_currency.clone())
                .await
                .map_err(|error| SyncError::database("snapshot: currency rates", error))?;
            for rate in &rates {
                records.push(record(
                    RecordKind::CurrencyRate,
                    &[&rate.from_currency, &rate.to_currency, &rate.date],
                    rate,
                )?);
            }
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        SqliteAccountRepository, SqliteFeeCatchUpRepository, SqliteFeeScheduleRepository,
        SqliteHoldingNoteRepository, SqliteHoldingRepository, SqliteTransactionRepository,
        UpdateFrequency,
    };
    use crate::context::asset::{
        AssetClass, CreateAssetDTO, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
        SqliteAssetRepository,
    };
    use crate::context::currency::{SqliteCurrencyPairRepository, SqliteCurrencyRateRepository};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    fn build(
        pool: &sqlx::Pool<sqlx::Sqlite>,
    ) -> (
        Arc<AccountService>,
        Arc<AssetService>,
        ServicePortfolioSnapshot,
    ) {
        let account_service = Arc::new(
            AccountService::new(
                Box::new(SqliteAccountRepository::new(pool.clone())),
                Box::new(SqliteHoldingRepository::new(pool.clone())),
                Box::new(SqliteTransactionRepository::new(pool.clone())),
            )
            .with_fee_schedule_repo(Box::new(SqliteFeeScheduleRepository::new(pool.clone())))
            .with_fee_catch_up_repo(Box::new(SqliteFeeCatchUpRepository::new(pool.clone())))
            .with_holding_note_repo(Box::new(SqliteHoldingNoteRepository::new(pool.clone()))),
        );
        let asset_service = Arc::new(AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ));
        let currency_service = Arc::new(CurrencyService::new(
            Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
            Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
        ));
        let snapshot = ServicePortfolioSnapshot::new(
            Arc::clone(&account_service),
            Arc::clone(&asset_service),
            currency_service,
        );
        (account_service, asset_service, snapshot)
    }

    // SYN-013/021/027 — the snapshot carries one record per user record, parents before
    // children, and never the system-seeded cash asset or cash category.
    #[tokio::test]
    async fn records_cover_user_records_and_skip_system_seeded_ones() {
        let pool = make_pool().await;
        let (account_service, asset_service, snapshot) = build(&pool);
        let asset = asset_service
            .create_asset(CreateAssetDTO {
                name: "AAPL".into(),
                reference: "AAPL".into(),
                isin: None,
                class: AssetClass::Stocks,
                currency: "USD".into(),
                risk_level: 2,
                category_id: SYSTEM_CATEGORY_ID.into(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        asset_service.seed_cash_asset("USD").await.unwrap();
        let account = account_service
            .create(
                "Portfolio".into(),
                String::new(),
                "USD".into(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        account_service
            .seed_cash_holding(&account.id)
            .await
            .unwrap();
        account_service
            .record_deposit(&account.id, "2026-01-01".into(), 1_000_000_000, None)
            .await
            .unwrap();

        let records = snapshot.records().await.unwrap();
        let kinds: Vec<RecordKind> = records.iter().map(|record| record.record_kind).collect();
        assert_eq!(
            kinds,
            vec![
                RecordKind::Asset,
                RecordKind::Account,
                RecordKind::Transaction
            ]
        );
        assert_eq!(records[0].record_identity.as_str(), asset.id);
        assert_eq!(records[1].record_identity.as_str(), account.id);
        assert!(
            records[1].content.contains("\"name\":\"Portfolio\""),
            "content is the record's own serialization"
        );
    }
}
