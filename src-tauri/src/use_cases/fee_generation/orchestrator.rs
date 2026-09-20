use super::error::FeeGenerationError;
use crate::context::account::{AccountError, AccountServiceContract, FeeFrequency, FeeSchedule};
use crate::core::logger::BACKEND;
use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

/// Last calendar day of `year`/`month` (the period boundary, FEE-042). `None` only at
/// chrono's representable ceiling — far beyond any real schedule date; callers skip.
fn last_day_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)?.pred_opt()
}

/// The period-boundary date (per `freq`) of the period that contains `date` (FEE-042).
fn period_end_containing(freq: FeeFrequency, date: NaiveDate) -> Option<NaiveDate> {
    match freq {
        FeeFrequency::Monthly => last_day_of_month(date.year(), date.month()),
        FeeFrequency::Quarterly => {
            let quarter_end_month = ((date.month() - 1) / 3) * 3 + 3; // 3, 6, 9, or 12
            last_day_of_month(date.year(), quarter_end_month)
        }
        FeeFrequency::Annually => last_day_of_month(date.year(), 12),
    }
}

/// The boundary of the period immediately following the one ending at `boundary`.
fn next_period_end(freq: FeeFrequency, boundary: NaiveDate) -> Option<NaiveDate> {
    period_end_containing(freq, boundary.succ_opt()?)
}

/// The sync surface `apply_due_fee_deductions` runs once before generation (SYN-060, D9):
/// an injected trait object over the sync service — never the sibling
/// `use_cases::portfolio_sync` orchestrator (B18's "MUST NOT import from another use
/// case") — wired from `lib.rs`.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait LaunchSyncSurface: Send + Sync {
    /// Runs one sync when sync is enabled and not paused; a no-op otherwise. Never fails
    /// outward — a failed launch sync is logged and generation proceeds regardless.
    async fn run_launch_sync(&self);
}

/// Orchestrates periodic management fee deduction across all active fee schedules
/// (FEE-040 — lazy catch-up generation).
pub struct FeeGenerationOrchestrator {
    account_service: Arc<dyn AccountServiceContract>,
    launch_sync: Option<Arc<dyn LaunchSyncSurface>>,
}

impl FeeGenerationOrchestrator {
    /// Creates a new orchestrator.
    pub fn new(account_service: Arc<dyn AccountServiceContract>) -> Self {
        Self {
            account_service,
            launch_sync: None,
        }
    }

    /// Attaches the launch-sync surface (SYN-060, D9): when present, `apply_due_fee_deductions`
    /// runs it once, before any generation, so generation sees the merged ledger.
    pub fn with_launch_sync(mut self, launch_sync: Arc<dyn LaunchSyncSurface>) -> Self {
        self.launch_sync = Some(launch_sync);
        self
    }

    /// Applies all due management fee deductions across every active fee schedule
    /// (FEE-040/041/042/043/044/045/047). Reuses `AccountService::record_management_fee`
    /// per due period — the per-period rate is `annual_rate ÷ periods_per_year`, which
    /// gives the sequential per-period reduction and the oversell guard for free.
    pub async fn apply_due_fee_deductions(&self) -> Result<(), FeeGenerationError> {
        // SYN-060 — one sync before any generation, so generation sees the merged ledger.
        if let Some(launch_sync) = &self.launch_sync {
            launch_sync.run_launch_sync().await;
        }
        let today = chrono::Local::now().date_naive();
        let schedules = self.account_service.list_active_fee_schedules().await?;
        // FEE-078 — schedules of accounts with management fees disabled are
        // paused: skipped without advancing the cursor, so re-enabling
        // backfills the paused periods on the next run. Each account is loaded
        // once, not once per schedule.
        let mut management_fees_enabled_by_account: HashMap<String, bool> = HashMap::new();
        // reviewer-backend FP: the Entry API would hold the map borrow across
        // the get_by_id await; contains_key + insert is clearer here (2026-07-05).
        for schedule in &schedules {
            if !management_fees_enabled_by_account.contains_key(&schedule.account_id) {
                let account = self.account_service.get_by_id(&schedule.account_id).await?;
                let enabled = account.is_some_and(|account| account.management_fees_enabled);
                management_fees_enabled_by_account.insert(schedule.account_id.clone(), enabled);
            }
        }
        for schedule in schedules {
            let enabled = management_fees_enabled_by_account
                .get(&schedule.account_id)
                .copied()
                .unwrap_or(false);
            if enabled {
                self.apply_schedule(&schedule, today).await?;
            }
        }
        Ok(())
    }

    /// Generates the due deductions for one active schedule up to `today` (FEE-040–047).
    async fn apply_schedule(
        &self,
        schedule: &FeeSchedule,
        today: NaiveDate,
    ) -> Result<(), FeeGenerationError> {
        let Ok(start) = NaiveDate::parse_from_str(&schedule.start_date, "%Y-%m-%d") else {
            return Ok(()); // malformed start_date — skip defensively
        };
        let cursor = schedule
            .last_applied_period
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        let end = schedule
            .end_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
        // FEE-041 — per-period rate is the annual rate scaled to the cadence.
        let per_period_percent =
            schedule.annual_rate_percent_micros / schedule.frequency.periods_per_year();

        let Some(mut boundary) = period_end_containing(schedule.frequency, start) else {
            return Ok(()); // unrepresentable boundary date — skip defensively
        };
        let mut last_processed: Option<NaiveDate> = None;
        // FEE-040 — only completed periods (boundary ≤ today, never future-dated).
        while boundary <= today {
            // FEE-045 — stop once the period boundary passes end_date.
            if let Some(end) = end {
                if boundary > end {
                    break;
                }
            }
            let after_cursor = cursor.is_none_or(|c| boundary > c);
            if boundary >= start && after_cursor {
                if per_period_percent > 0 {
                    let dated = boundary.format("%Y-%m-%d").to_string();
                    let deduction_id = deterministic_deduction_id(
                        &schedule.account_id,
                        &schedule.asset_id,
                        &dated,
                    );
                    let result = self
                        .account_service
                        .record_generated_management_fee(
                            &schedule.account_id,
                            schedule.asset_id.clone(),
                            dated,
                            per_period_percent,
                            deduction_id,
                        )
                        .await;
                    match result {
                        Ok(_) => {}
                        // FEE-047 — holding qty is 0 or the removal rounds to 0: skip.
                        Err(AccountError::QuantityNotPositive) => {}
                        // FEE-044/047 — a backfilled deduction that would oversell: skip.
                        Err(AccountError::CascadingOversell) => {}
                        // CFR-016/FEE-047 — the user removed this deduction on some device;
                        // the regeneration is outranked and the period stands skipped.
                        Err(AccountError::ApplicationWriteOutranked) => {
                            tracing::info!(
                                target: BACKEND,
                                account_id = %schedule.account_id,
                                asset_id = %schedule.asset_id,
                                "fee generation: period skipped, the user's removal outranks the regeneration"
                            );
                        }
                        Err(other) => return Err(other.into()),
                    }
                }
                // FEE-043 — cursor advances for every processed period, skipped or not.
                last_processed = Some(boundary);
            }
            let Some(next) = next_period_end(schedule.frequency, boundary) else {
                break; // unrepresentable next boundary — stop iterating
            };
            boundary = next;
        }

        if let Some(last) = last_processed {
            self.account_service
                .advance_fee_schedule_cursor(
                    &schedule.account_id,
                    &schedule.asset_id,
                    last.format("%Y-%m-%d").to_string(),
                )
                .await?;
        }
        Ok(())
    }
}

/// FEE-048 — the deterministic identity of a generated deduction: derived from the holding
/// it charges (account, asset) and its period boundary, never from the schedule's own id, so
/// two devices generating the same period converge on one transaction record (CFR-034/043).
/// One-off deductions (FEE-02x) keep a freshly generated uuid v4 — this function is never
/// called for them.
fn deterministic_deduction_id(account_id: &str, asset_id: &str, period_boundary: &str) -> String {
    let digest = Sha256::digest(format!("{account_id}|{asset_id}|{period_boundary}"));
    let hex = format!("{digest:x}");
    format!("fee-{}", &hex[..32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        Account, AccountService, SqliteAccountRepository, SqliteFeeCatchUpRepository,
        SqliteFeeScheduleRepository, SqliteHoldingRepository, SqliteTransactionRepository,
        TransactionType, UpdateFrequency,
    };
    use crate::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> sqlx::Pool<sqlx::Sqlite> {
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

    fn make_account_service(pool: &sqlx::Pool<sqlx::Sqlite>) -> Arc<AccountService> {
        Arc::new(
            AccountService::new(
                Box::new(SqliteAccountRepository::new(pool.clone())),
                Box::new(SqliteHoldingRepository::new(pool.clone())),
                Box::new(SqliteTransactionRepository::new(pool.clone())),
            )
            .with_fee_schedule_repo(Box::new(SqliteFeeScheduleRepository::new(pool.clone())))
            .with_fee_catch_up_repo(Box::new(SqliteFeeCatchUpRepository::new(pool.clone()))),
        )
    }

    fn make_asset_service(pool: &sqlx::Pool<sqlx::Sqlite>) -> Arc<AssetService> {
        Arc::new(AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ))
    }

    fn micro(v: i64) -> i64 {
        v * 1_000_000
    }

    async fn enable_management_fees(svc: &AccountService, account: &Account) -> Account {
        svc.update(
            account.id.clone(),
            account.name.clone(),
            String::new(),
            account.currency.clone(),
            account.update_frequency,
            true,
        )
        .await
        .unwrap()
    }

    async fn seed_stock(asset_svc: &AssetService) -> String {
        asset_svc
            .create_asset(CreateAssetDTO {
                name: "Fee Stock".to_string(),
                reference: "FEES".to_string(),
                isin: None,
                class: AssetClass::Stocks,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap()
            .id
    }

    async fn seed_cash(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        account_svc: &AccountService,
        account_id: &str,
    ) {
        let cash_asset_id = "system-cash-eur".to_string();
        sqlx::query(
            "INSERT OR IGNORE INTO categories (id, name, is_deleted) VALUES ('system-cash-category', 'cash', 0)",
        )
        .execute(pool)
        .await
        .expect("seed cash category");
        sqlx::query(
            "INSERT OR IGNORE INTO assets (id, name, reference, asset_class, category_id, currency, risk_level) \
             VALUES (?, ?, ?, 'Cash', 'system-cash-category', ?, 1)",
        )
        .bind(&cash_asset_id)
        .bind("Cash EUR")
        .bind("EUR")
        .bind("EUR")
        .execute(pool)
        .await
        .expect("seed cash asset");
        account_svc
            .record_deposit(
                account_id,
                "2020-01-01".to_string(),
                1_000_000_000_000,
                None,
            )
            .await
            .expect("seed cash deposit");
    }

    // FEE-040 — apply_due_fee_deductions: no schedules → no transactions created.
    #[tokio::test]
    async fn fee_040_no_schedules_no_transactions_created() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let account = account_svc
            .create(
                "Test".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        // Expect todo!() panic for now — this is the red baseline.
        let result = uc.apply_due_fee_deductions().await;
        // Once implemented: no schedules → no error.
        // Until then, the todo!() panic IS the red signal.
        assert!(
            result.is_ok(),
            "with no schedules, apply_due_fee_deductions must succeed: {:?}",
            result
        );
        // No ManagementFee transactions should have been created.
        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        assert!(
            txs.iter()
                .all(|t| t.transaction_type != TransactionType::ManagementFee),
            "no ManagementFee transactions should exist"
        );
    }

    // FEE-041 — apply_due_fee_deductions generates one deduction per completed period.
    #[tokio::test]
    async fn fee_041_generates_one_deduction_per_completed_period() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-041".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();

        // Create a monthly schedule starting 2024-01-01.
        // As of 2024-04-01 (3 complete months have passed: Jan, Feb, Mar).
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                1_000_000, // 1% annual
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        uc.apply_due_fee_deductions().await.unwrap();

        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let fee_txs: Vec<_> = txs
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .collect();
        // FEE-041 — one deduction per completed period.
        assert!(
            !fee_txs.is_empty(),
            "at least one ManagementFee transaction must be created for completed periods"
        );
    }

    // FEE-043 — last_applied_period advances even when a period is skipped.
    #[tokio::test]
    async fn fee_043_cursor_advances_for_skipped_periods() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-043".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;

        // Create a schedule for a stock that is NOT held (so all periods skip).
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                1_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        uc.apply_due_fee_deductions().await.unwrap();

        // Cursor must have advanced past start_date even though all periods were skipped.
        let schedule = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .expect("schedule must still exist after skipped periods");
        assert!(
            schedule.last_applied_period.is_some(),
            "last_applied_period must advance even when all periods skipped (FEE-043)"
        );
    }

    // FEE-047 — period where holding qty is 0 is skipped (not an error).
    #[tokio::test]
    async fn fee_047_zero_holding_quantity_skips_period() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-047".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;

        // Schedule exists but the asset is never purchased → qty_as_of = 0 for all periods.
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                1_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        // Must succeed (not an error) even though all periods are skipped.
        uc.apply_due_fee_deductions().await.unwrap();

        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let fee_txs: Vec<_> = txs
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .collect();
        assert!(
            fee_txs.is_empty(),
            "no ManagementFee transactions when holding qty was 0 for all periods (FEE-047)"
        );
    }

    // FEE-070 — apply_due_fee_deductions is idempotent: re-running on the same
    // state produces no additional transactions.
    #[tokio::test]
    async fn fee_070_apply_due_fee_deductions_is_idempotent() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-070".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                1_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        uc.apply_due_fee_deductions().await.unwrap();

        let txs_after_first = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let count_after_first = txs_after_first
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .count();

        // Run again — must not create additional transactions.
        uc.apply_due_fee_deductions().await.unwrap();

        let txs_after_second = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let count_after_second = txs_after_second
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .count();

        assert_eq!(
            count_after_first, count_after_second,
            "second run must not create additional ManagementFee transactions (FEE-070)"
        );
    }

    // FEE-044 — catch-up across long absences: a schedule whose start_date is several
    // periods in the past backfills ALL elapsed completed periods at once on a single
    // run, each dated at its own boundary, applied sequentially (FEE-041 compounding).
    #[tokio::test]
    async fn fee_044_backfills_all_elapsed_periods_sequentially() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-044".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        // 12% annual, monthly cadence → 1% per period (FEE-041).
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                12_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        let schedule = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .expect("schedule must exist");
        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        // Deterministic "today" mid-April 2024 → Jan, Feb, Mar are the only completed periods.
        let today = chrono::NaiveDate::from_ymd_opt(2024, 4, 15).expect("valid date");
        uc.apply_schedule(&schedule, today).await.unwrap();

        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let mut fee_txs: Vec<_> = txs
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .collect();
        fee_txs.sort_by(|a, b| a.date.cmp(&b.date));

        // FEE-044 — one deduction per completed elapsed period (Jan, Feb, Mar).
        assert_eq!(
            fee_txs.len(),
            3,
            "expected 3 backfilled deductions, got {}",
            fee_txs.len()
        );
        // FEE-042 — each dated at its own period boundary.
        assert_eq!(
            fee_txs.iter().map(|t| t.date.as_str()).collect::<Vec<_>>(),
            vec!["2024-01-31", "2024-02-29", "2024-03-31"],
            "deductions must be dated at month-end boundaries (FEE-042)"
        );
        // FEE-041 — sequential per-period reduction compounds (each on the prior reduced base):
        // floor(100 ×1%)=1_000_000; floor(99 ×1%)=990_000; floor(98.01 ×1%)=980_100.
        assert_eq!(
            fee_txs.iter().map(|t| t.quantity).collect::<Vec<_>>(),
            vec![1_000_000, 990_000, 980_100],
            "per-period removals must compound sequentially (FEE-041)"
        );

        // FEE-043 — cursor lands on the last completed period boundary.
        let after = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .expect("schedule must still exist");
        assert_eq!(
            after.last_applied_period.as_deref(),
            Some("2024-03-31"),
            "cursor must advance to the last completed period (FEE-043)"
        );
    }

    // FEE-045 — no deductions are generated for periods whose boundary is after end_date.
    #[tokio::test]
    async fn fee_045_no_deductions_past_end_date() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-045-end".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        // end_date 2024-02-15 → only the Jan-31 boundary is on/before it; Feb-29 is after.
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                12_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                Some("2024-02-15".to_string()),
            )
            .await
            .unwrap();

        let schedule = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .expect("schedule must exist");
        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        let today = chrono::NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date");
        uc.apply_schedule(&schedule, today).await.unwrap();

        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let fee_txs: Vec<_> = txs
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .collect();

        // FEE-045 — only the Jan period (boundary ≤ end_date) is generated.
        assert_eq!(
            fee_txs.len(),
            1,
            "only periods with boundary ≤ end_date generate (FEE-045), got {}",
            fee_txs.len()
        );
        assert_eq!(
            fee_txs[0].date.as_str(),
            "2024-01-31",
            "the single deduction must be the Jan-31 period"
        );
        let after = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .expect("schedule must still exist");
        assert_eq!(
            after.last_applied_period.as_deref(),
            Some("2024-01-31"),
            "cursor stops at the last in-window period boundary (FEE-045)"
        );
    }

    // FEE-045/061 — an inactive schedule generates nothing (not listed for generation).
    #[tokio::test]
    async fn fee_045_inactive_schedule_generates_nothing() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-045-inactive".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                12_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();
        // FEE-061 — deactivate the schedule.
        account_svc
            .update_fee_schedule(&account.id, &stock_id, 12_000_000, None, false)
            .await
            .unwrap();

        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        uc.apply_due_fee_deductions().await.unwrap();

        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let fee_txs: Vec<_> = txs
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .collect();
        // FEE-045 — nothing generated while inactive.
        assert!(
            fee_txs.is_empty(),
            "inactive schedule must generate no deductions (FEE-045/061)"
        );
        // The cursor never advanced (the schedule was not processed at all).
        let after = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .expect("schedule must still exist");
        assert!(!after.active, "schedule must remain inactive");
        assert!(
            after.last_applied_period.is_none(),
            "cursor must not advance for an inactive schedule (FEE-045)"
        );
    }

    // FEE-047 — oversell-skip subbranch: a backfilled period dated before an already-recorded
    // Sell that the removal would starve is SKIPPED (no FeeDeduction), the cursor advances past
    // it, and generation continues to later valid periods.
    #[tokio::test]
    async fn fee_047_oversell_period_is_skipped_and_generation_continues() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-047-oversell".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        // Buy 100, sell ALL 100 mid-March, then re-buy 100 in April.
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .sell_holding(
                &account.id,
                stock_id.clone(),
                "2024-03-15".to_string(),
                micro(100),
                micro(60),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-04-10".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                12_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        let schedule = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .expect("schedule must exist");
        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        let today = chrono::NaiveDate::from_ymd_opt(2024, 6, 15).expect("valid date");
        // FEE-047 — must not block / error even though Jan & Feb would oversell the Mar-15 sell.
        uc.apply_schedule(&schedule, today).await.unwrap();

        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let mut fee_txs: Vec<_> = txs
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .collect();
        fee_txs.sort_by(|a, b| a.date.cmp(&b.date));

        // Jan-31 and Feb-29 are skipped (removing 1 share would starve the 100-share Mar-15 sell);
        // Mar-31 is skipped (holding is 0 after the sell); Apr-30 and May-31 generate normally.
        assert_eq!(
            fee_txs.iter().map(|t| t.date.as_str()).collect::<Vec<_>>(),
            vec!["2024-04-30", "2024-05-31"],
            "oversell periods must be skipped; only post-rebuy periods generate (FEE-047)"
        );
        // FEE-041 — Apr removes 1% of 100, May removes 1% of the reduced 99.
        assert_eq!(
            fee_txs.iter().map(|t| t.quantity).collect::<Vec<_>>(),
            vec![1_000_000, 990_000],
            "surviving deductions follow sequential per-period reduction (FEE-041)"
        );
        // FEE-043 — the cursor still advances past every period, skipped or not.
        let after = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .expect("schedule must still exist");
        assert_eq!(
            after.last_applied_period.as_deref(),
            Some("2024-05-31"),
            "cursor advances past skipped oversell periods (FEE-043/047)"
        );
    }

    // FEE-078 — schedules of a disabled account are paused (skipped, cursor
    // untouched); re-enabling backfills the paused periods on the next run.
    #[tokio::test]
    async fn fee_078_disabled_account_pauses_generation_and_reenabling_backfills() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "FEE-078".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                12_000_000,
                FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        // Disable the mechanism, then run the catch-up: nothing generates.
        account_svc
            .update(
                account.id.clone(),
                account.name.clone(),
                String::new(),
                account.currency.clone(),
                account.update_frequency,
                false,
            )
            .await
            .unwrap();
        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        uc.apply_due_fee_deductions().await.unwrap();
        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        assert!(
            txs.iter()
                .all(|t| t.transaction_type != TransactionType::ManagementFee),
            "paused account must generate nothing"
        );
        let schedule = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            schedule.last_applied_period, None,
            "cursor must not advance while paused"
        );

        // Re-enable and run again: the paused periods backfill.
        enable_management_fees(&account_svc, &account).await;
        uc.apply_due_fee_deductions().await.unwrap();
        let txs = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap();
        let fee_count = txs
            .iter()
            .filter(|t| t.transaction_type == TransactionType::ManagementFee)
            .count();
        assert!(fee_count > 0, "re-enabling must backfill paused periods");
        let schedule = account_svc
            .get_fee_schedule(&account.id, &stock_id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            schedule.last_applied_period.is_some(),
            "cursor advances after backfill"
        );
    }

    // FEE-048/CFR-034/043 — same (account, asset, period) always yields the same id, so two
    // devices generating the same period independently converge on one transaction.
    #[test]
    fn deterministic_deduction_id_is_stable_for_the_same_holding_and_period() {
        let first = deterministic_deduction_id("acc-1", "asset-1", "2026-08-31");
        let second = deterministic_deduction_id("acc-1", "asset-1", "2026-08-31");
        assert_eq!(first, second);
    }

    // FEE-048 — a different period boundary yields a different id.
    #[test]
    fn deterministic_deduction_id_differs_for_a_different_period() {
        let august = deterministic_deduction_id("acc-1", "asset-1", "2026-08-31");
        let september = deterministic_deduction_id("acc-1", "asset-1", "2026-09-30");
        assert_ne!(august, september);
    }

    // FEE-048 — a different holding (account or asset) yields a different id for the same
    // period.
    #[test]
    fn deterministic_deduction_id_differs_for_a_different_holding() {
        let one = deterministic_deduction_id("acc-1", "asset-1", "2026-08-31");
        let other = deterministic_deduction_id("acc-2", "asset-1", "2026-08-31");
        assert_ne!(one, other);
    }

    // CFR-016 — a generated fee deduction is written by the application, never the user;
    // its change must carry `Origin::Application`, not the transaction repository's current
    // hardcoded `Origin::User`. This is what lets the user's deletion of a generated
    // deduction permanently outrank a later re-generation (CFR-016's own scenario).
    #[tokio::test]
    async fn generated_fee_deduction_is_recorded_with_application_origin() {
        let pool = setup_pool().await;
        sqlx::query(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, 'desktop-device', 'Desktop', '/tmp/sync', '2026-08-22T00:00:00Z', 0,
                       '2026-08-22T00:00:00Z', 0, X'00', 1)"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let recorder: Arc<dyn crate::shared::infrastructure::change_recorder::ChangeRecorder> =
            Arc::new(crate::context::sync::SqliteChangeRecorder::new(
                pool.clone(),
            ));
        let account_svc = Arc::new(
            AccountService::new(
                Box::new(
                    SqliteAccountRepository::new(pool.clone())
                        .with_change_recorder(recorder.clone()),
                ),
                Box::new(SqliteHoldingRepository::new(pool.clone())),
                Box::new(
                    SqliteTransactionRepository::new(pool.clone())
                        .with_change_recorder(recorder.clone()),
                ),
            )
            .with_fee_schedule_repo(Box::new(
                SqliteFeeScheduleRepository::new(pool.clone())
                    .with_change_recorder(recorder.clone()),
            ))
            .with_fee_catch_up_repo(Box::new(
                SqliteFeeCatchUpRepository::new(pool.clone()).with_change_recorder(recorder),
            )),
        );
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "Origin Test".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id,
                1_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                None,
            )
            .await
            .unwrap();

        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        uc.apply_due_fee_deductions().await.unwrap();

        // The seed deposit and buy are the user's own changes; only the deductions count.
        let origins: Vec<String> = sqlx::query_scalar(
            "SELECT origin FROM changes
             WHERE record_kind = 'Transaction' AND content LIKE '%\"ManagementFee\"%'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(
            !origins.is_empty(),
            "at least one generated deduction must have been recorded"
        );
        assert!(
            origins.iter().all(|origin| origin == "Application"),
            "CFR-016: every generated deduction must carry Origin::Application, got {origins:?}"
        );
    }

    // CFR-016/FEE-047 — a deduction the user removed is never regenerated: the recorder
    // refuses the application's write against the user's tombstone, the repository's
    // transaction rolls back, and generation treats the period as skipped — the run
    // succeeds, the deduction stays gone, and the cursor still advances (FEE-043).
    #[tokio::test]
    async fn regeneration_over_a_user_removed_deduction_is_a_skipped_period() {
        let pool = setup_pool().await;
        sqlx::query(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, 'desktop-device', 'Desktop', '/tmp/sync', '2026-08-22T00:00:00Z', 0,
                       '2026-08-22T00:00:00Z', 0, X'00', 1)"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let recorder: Arc<dyn crate::shared::infrastructure::change_recorder::ChangeRecorder> =
            Arc::new(crate::context::sync::SqliteChangeRecorder::new(
                pool.clone(),
            ));
        let account_svc = Arc::new(
            AccountService::new(
                Box::new(
                    SqliteAccountRepository::new(pool.clone())
                        .with_change_recorder(recorder.clone()),
                ),
                Box::new(SqliteHoldingRepository::new(pool.clone())),
                Box::new(
                    SqliteTransactionRepository::new(pool.clone())
                        .with_change_recorder(recorder.clone()),
                ),
            )
            .with_fee_schedule_repo(Box::new(
                SqliteFeeScheduleRepository::new(pool.clone())
                    .with_change_recorder(recorder.clone()),
            ))
            .with_fee_catch_up_repo(Box::new(
                SqliteFeeCatchUpRepository::new(pool.clone()).with_change_recorder(recorder),
            )),
        );
        let asset_svc = make_asset_service(&pool);
        let stock_id = seed_stock(&asset_svc).await;
        let account = account_svc
            .create(
                "Outranked".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let account = enable_management_fees(&account_svc, &account).await;
        seed_cash(&pool, &account_svc, &account.id).await;
        account_svc
            .buy_holding(
                &account.id,
                stock_id.clone(),
                "2024-01-01".to_string(),
                micro(100),
                micro(50),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();
        // One completed period only: January 2024.
        account_svc
            .create_fee_schedule(
                &account.id,
                stock_id.clone(),
                12_000_000,
                crate::context::account::FeeFrequency::Monthly,
                "2024-01-01".to_string(),
                Some("2024-01-31".to_string()),
            )
            .await
            .unwrap();
        let uc = FeeGenerationOrchestrator::new(account_svc.clone());
        uc.apply_due_fee_deductions().await.unwrap();
        let deduction = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.transaction_type == TransactionType::ManagementFee)
            .expect("January's deduction must have been generated");

        // The user removes it, then the cursor is lost (another device's view, say).
        account_svc
            .cancel_transaction(&account.id, &deduction.id)
            .await
            .unwrap();
        sqlx::query("DELETE FROM fee_catch_up_positions WHERE account_id = ?")
            .bind(&account.id)
            .execute(&pool)
            .await
            .unwrap();

        uc.apply_due_fee_deductions().await.expect(
            "CFR-016/FEE-047: an outranked regeneration is a skipped period, not a failure",
        );

        let regenerated = account_svc
            .get_all_transactions_for_account(&account.id)
            .await
            .unwrap()
            .into_iter()
            .any(|t| t.transaction_type == TransactionType::ManagementFee);
        assert!(
            !regenerated,
            "CFR-016: the user's removal outranks the regeneration — the deduction stays gone"
        );
        let tombstones: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tombstones WHERE record_kind = 'Transaction' AND record_identity = ?",
        )
        .bind(&deduction.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            tombstones, 1,
            "the user's tombstone must survive the refused write"
        );
        let cursor: Option<String> = sqlx::query_scalar(
            "SELECT last_applied_period FROM fee_catch_up_positions WHERE account_id = ?",
        )
        .bind(&account.id)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(
            cursor.as_deref(),
            Some("2024-01-31"),
            "FEE-043: the cursor advances past the skipped period"
        );
    }

    // SYN-060/D9 — apply_due_fee_deductions runs the launch sync exactly once, before any
    // generation, when a launch-sync surface is attached.
    #[tokio::test]
    async fn apply_due_fee_deductions_runs_the_launch_sync_before_generation() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let mut launch_sync = MockLaunchSyncSurface::new();
        launch_sync
            .expect_run_launch_sync()
            .times(1)
            .returning(|| ());
        let uc =
            FeeGenerationOrchestrator::new(account_svc).with_launch_sync(Arc::new(launch_sync));

        uc.apply_due_fee_deductions()
            .await
            .expect("must not error even with no schedules");
        // The MockLaunchSyncSurface panics on drop if `run_launch_sync` was not called
        // exactly once (mockall's unmet-expectation check) — SYN-060's ordering guarantee.
    }
}
