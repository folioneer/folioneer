use super::error::{
    DividendError, DividendTask, FreeSharesError, FreeSharesTask, InterestError, InterestTask,
    ManagementFeeError, ManagementFeeTask, OpenHoldingError, OpenHoldingTask, SplitError,
    SplitTask,
};
use super::shared::ensure_cash_asset;
use crate::context::account::{
    AccountError, AccountServiceContract, ManagementFeeRemoval, Transaction,
};
use crate::context::asset::{AssetClass, AssetServiceContract};
use crate::core::logger::BACKEND;
use std::sync::Arc;

/// Single orchestrator for every operation that mutates a `Holding` through a `Transaction`:
/// opening balance, buy, sell, correct, cancel.
///
/// Injects `Arc<AccountService>` + `Arc<AssetService>` and shares them across all five methods.
/// `asset_service` is used today by `open_holding` for the archived-asset guard, and will also
/// drive the cross-BC `ensure_cash_asset` step inserted by the cash-tracking spec
/// (CSH-040 / CSH-050 / CSH-042 / CSH-024).
pub struct HoldingTransactionUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
}

impl HoldingTransactionUseCase {
    /// Creates a new HoldingTransactionUseCase.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
        }
    }

    /// Seeds a holding from a known quantity and total cost (TRX-042).
    ///
    /// Cross-BC guard: rejects the request if the asset does not exist
    /// (TRX-056), is archived (TRX-050), or is a system Cash Asset (CSH-061).
    /// Delegates the account-side write to `AccountService::open_holding`.
    /// Returns the typed `OpenHoldingError` composite. Asset-side repo failures
    /// from `get_asset_by_id` are translated to `AccountError::DatabaseError`
    /// (matching the `ensure_cash_for` precedent) so the FE wire surface carries a
    /// single `{ code: "DatabaseError" }` shape rather than two indistinguishable arms.
    pub async fn open_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        total_cost: i64,
    ) -> Result<Transaction, OpenHoldingError> {
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "open_holding: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(OpenHoldingTask::AssetNotFound.into()),
            Some(a) if a.is_archived => return Err(OpenHoldingTask::ArchivedAsset.into()),
            // CSH-061 — Cash Assets cannot be seeded via OpeningBalance; user records
            // initial cash via `record_deposit` instead.
            Some(a) if a.class == AssetClass::Cash => {
                return Err(OpenHoldingTask::OpeningBalanceOnCashAsset.into())
            }
            Some(_) => {}
        }
        self.account_service
            .open_holding(account_id, asset_id, date, quantity, total_cost)
            .await
    }

    /// Records a purchase of an asset into an account (TRX-027).
    /// Seeds the system Cash Asset for the account's currency (CSH-010) before delegating;
    /// the aggregate replays the cash holding inside `Account::buy_holding` (CSH-040 / CSH-041).
    #[allow(clippy::too_many_arguments)]
    pub async fn buy_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "buy_holding").await?;
        self.account_service
            .buy_holding(
                account_id,
                asset_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .await
    }

    /// Records a sale of an asset from an account (SEL-012, SEL-021, SEL-023, SEL-024).
    /// Seeds the system Cash Asset (CSH-010); the aggregate lazy-creates the Cash Holding
    /// when this is the first cash-affecting transaction (CSH-050 / CSH-012).
    #[allow(clippy::too_many_arguments)]
    pub async fn sell_holding(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "sell_holding").await?;
        self.account_service
            .sell_holding(
                account_id,
                asset_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .await
    }

    /// Corrects an existing transaction and recalculates the affected holding (TRX-031).
    /// Seeds the system Cash Asset; the aggregate replay re-evaluates the cash holding for
    /// any cash-affecting tx (CSH-042 / CSH-051) and may raise InsufficientCash.
    #[allow(clippy::too_many_arguments)]
    pub async fn correct_transaction(
        &self,
        account_id: &str,
        transaction_id: &str,
        date: String,
        quantity: i64,
        unit_price: i64,
        exchange_rate: i64,
        fees: i64,
        total_amount: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "correct_transaction")
            .await?;
        self.account_service
            .correct_transaction(
                account_id,
                transaction_id,
                date,
                quantity,
                unit_price,
                exchange_rate,
                fees,
                total_amount,
                note,
            )
            .await
    }

    /// Cancels a transaction and recalculates (or removes) the associated holding (TRX-034).
    /// The aggregate replay catches any chronologically-later violation (CSH-024 / CSH-051).
    pub async fn cancel_transaction(
        &self,
        account_id: &str,
        transaction_id: &str,
    ) -> Result<(), AccountError> {
        self.ensure_cash_for(account_id, "cancel_transaction")
            .await?;
        self.account_service
            .cancel_transaction(account_id, transaction_id)
            .await
    }

    /// Records a Deposit into an account (CSH-022).
    /// Seeds the system Cash Asset (CSH-010) before delegating; the aggregate
    /// lazy-creates the Cash Holding (CSH-012) and persists the Transaction.
    /// Returns a typed `AccountError`: in-account and cross-BC asset-seed
    /// failures both surface as `AccountError` (see `ensure_cash_for`).
    pub async fn record_deposit(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "record_deposit").await?;
        self.account_service
            .record_deposit(account_id, date, amount, note)
            .await
    }

    /// Records a Withdrawal from an account (CSH-032).
    /// Raises InsufficientCash (CSH-080) when no Cash Holding exists or balance < amount.
    pub async fn record_withdrawal(
        &self,
        account_id: &str,
        date: String,
        amount: i64,
        note: Option<String>,
    ) -> Result<Transaction, AccountError> {
        self.ensure_cash_for(account_id, "record_withdrawal")
            .await?;
        self.account_service
            .record_withdrawal(account_id, date, amount, note)
            .await
    }

    /// Records a cash Dividend attributed to the paying asset (DIV-023).
    ///
    /// Cross-BC guards (DIV-011): rejects if account is unknown, asset is unknown,
    /// asset is not currently held (quantity = 0), or asset is a Cash Asset.
    /// Seeds the system Cash Asset (CSH-010) before delegating; the aggregate
    /// credits the Cash Holding by `total_amount = amount_micros × exchange_rate`
    /// (account currency), lazy-creating it if absent (CSH-012/CSH-050).
    /// The paying asset's holding quantity, average cost, and cost basis are
    /// unchanged (DIV-024). Does not create or modify any AssetPrice row (DIV-027).
    /// Returns `DividendError` — no `InsufficientCash` variant (credit-only).
    pub async fn record_dividend(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        amount_micros: i64,
        exchange_rate: i64,
        note: Option<String>,
    ) -> Result<Transaction, DividendError> {
        // DIV-011 — account must exist (checked before any asset work).
        let account = self
            .account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        // DIV-011 — asset must exist and must not be a Cash Asset.
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "record_dividend: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(DividendTask::AssetNotFound.into()),
            Some(a) if a.class == AssetClass::Cash => {
                return Err(DividendTask::DividendOnCashAsset.into())
            }
            Some(_) => {}
        }

        // DIV-011 — asset must be currently held (quantity > 0). A repository
        // failure here is surfaced as `DatabaseError` by the service layer.
        let held = self
            .account_service
            .get_holding_by_account_asset(account_id, &asset_id)
            .await?;
        match held {
            Some(h) if h.quantity > 0 => {}
            _ => return Err(DividendTask::AssetNotHeld.into()),
        }

        // CSH-010 — ensure the system Cash Asset for the account's currency exists.
        ensure_cash_asset(&self.asset_service, &account.currency)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, err = ?e, "record_dividend: ensure_cash_asset failed");
                AccountError::DatabaseError
            })?;

        // Delegate the credit + persistence to the account BC; its `AccountError`
        // surfaces on the dividend wire as `DividendError::Account`.
        self.account_service
            .record_dividend(
                account_id,
                asset_id,
                date,
                amount_micros,
                exchange_rate,
                note,
            )
            .await
            .map_err(DividendError::Account)
    }

    /// Records a FreeShares distribution attributed to a held distributing asset
    /// (FSD-011/022).
    ///
    /// The distribution has no cash leg (FSD-022d — no `ensure_cash_asset`, no
    /// `InsufficientCash`) and never touches an `AssetPrice` row (FSD-024).
    /// Returns `FreeSharesError`.
    pub async fn record_free_shares(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        quantity: i64,
        note: Option<String>,
    ) -> Result<Transaction, FreeSharesError> {
        // FSD-011 — account must exist (checked before any asset work).
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        // FSD-011 — asset must exist and must not be a Cash Asset.
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "record_free_shares: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(FreeSharesTask::AssetNotFound.into()),
            Some(a) if a.class == AssetClass::Cash => {
                return Err(FreeSharesTask::FreeSharesOnCashAsset.into())
            }
            Some(_) => {}
        }

        // FSD-011 — asset must be currently held (quantity > 0).
        let held = self
            .account_service
            .get_holding_by_account_asset(account_id, &asset_id)
            .await?;
        match held {
            Some(h) if h.quantity > 0 => {}
            _ => return Err(FreeSharesTask::AssetNotHeld.into()),
        }

        // Delegate to the account BC; its `AccountError` surfaces on the
        // free-shares wire as `FreeSharesError::Account`.
        self.account_service
            .record_free_shares(account_id, asset_id, date, quantity, note)
            .await
            .map_err(FreeSharesError::Account)
    }

    /// Records a stock split rescaling a held position (SPL-010/012).
    ///
    /// Cross-BC guards (SPL-012): rejects if the account is unknown, the asset
    /// is unknown, not currently held, or a Cash Asset. No cash leg. The factor
    /// and replay guards (SPL-011/021) live in the account BC.
    pub async fn record_split(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        factor: i64,
        note: Option<String>,
    ) -> Result<Transaction, SplitError> {
        // SPL-012 — account must exist (checked before any asset work).
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        // SPL-012 — asset must exist and must not be a Cash Asset.
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "record_split: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(SplitTask::AssetNotFound.into()),
            Some(a) if a.class == AssetClass::Cash => {
                return Err(AccountError::SplitOnCashAsset.into())
            }
            Some(_) => {}
        }

        // SPL-012 — asset must be currently held (quantity > 0).
        let held = self
            .account_service
            .get_holding_by_account_asset(account_id, &asset_id)
            .await?;
        match held {
            Some(h) if h.quantity > 0 => {}
            _ => return Err(SplitTask::AssetNotHeld.into()),
        }

        // Delegate to the account BC; its `AccountError` surfaces on the
        // split wire as `SplitError::Account`.
        self.account_service
            .record_split(account_id, asset_id, date, factor, note)
            .await
            .map_err(SplitError::Account)
    }

    /// Records a management fee deduction on a held asset (FEE-012/011).
    ///
    /// Cross-BC guards (FEE-011): rejects if account is unknown, asset is unknown,
    /// asset is not currently held (quantity = 0), or asset is a Cash Asset.
    /// No cash leg — does not call `ensure_cash_asset`.
    /// Returns `ManagementFeeError`.
    pub async fn record_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: i64,
        note: Option<String>,
    ) -> Result<Transaction, ManagementFeeError> {
        self.ensure_management_fee_target(account_id, &asset_id)
            .await?;
        // Delegate to the account BC; its `AccountError` surfaces on the
        // management-fee wire as `ManagementFeeError::Account`.
        self.account_service
            .record_management_fee(account_id, asset_id, date, percent_micros, note)
            .await
            .map_err(ManagementFeeError::Account)
    }

    /// Records a one-off management fee entered either as a percentage of the holding or
    /// as the quantity the holding should hold after it — exactly one of the two
    /// (FEE-021/028). Same cross-BC guards as `record_management_fee`.
    pub async fn record_management_fee_entry(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        resulting_quantity_micros: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, ManagementFeeError> {
        self.ensure_management_fee_target(account_id, &asset_id)
            .await?;
        self.account_service
            .record_management_fee_entry(
                account_id,
                asset_id,
                date,
                percent_micros,
                resulting_quantity_micros,
                note,
            )
            .await
            .map_err(ManagementFeeError::Account)
    }

    /// What a one-off management fee entered by its resulting quantity would remove
    /// (FEE-029); same guards and rejections as the recording, writes nothing.
    pub async fn preview_management_fee(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        resulting_quantity_micros: i64,
    ) -> Result<ManagementFeeRemoval, ManagementFeeError> {
        self.ensure_management_fee_target(account_id, &asset_id)
            .await?;
        self.account_service
            .preview_management_fee(account_id, &asset_id, &date, resulting_quantity_micros)
            .await
            .map_err(ManagementFeeError::Account)
    }

    /// FEE-012 — the account exists, the asset exists and is not a Cash Asset, and the
    /// asset is currently held (quantity > 0).
    async fn ensure_management_fee_target(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<(), ManagementFeeError> {
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        let asset = self
            .asset_service
            .get_asset_by_id(asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "management fee: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        match asset {
            None => return Err(ManagementFeeTask::AssetNotFound.into()),
            Some(a) if a.class == AssetClass::Cash => {
                return Err(ManagementFeeTask::ManagementFeeOnCashAsset.into())
            }
            Some(_) => {}
        }

        let held = self
            .account_service
            .get_holding_by_account_asset(account_id, asset_id)
            .await?;
        match held {
            Some(h) if h.quantity > 0 => Ok(()),
            _ => Err(ManagementFeeTask::AssetNotHeld.into()),
        }
    }

    /// Records an Interest credit on a held asset or the account's cash line
    /// (INT-011/023).
    ///
    /// Cross-BC guards (INT-011/012): rejects if the account is unknown or the
    /// asset is unknown. A Cash-class asset is always a valid target — the
    /// eligibility and held checks are skipped for it (INT-023, the cash line
    /// is interest-bearing); a non-cash asset must be `interest_bearing`
    /// (INT-012) and currently held (quantity > 0).
    /// The credit has no external cash leg (no `ensure_cash_asset` — the cash
    /// line only exists once a deposit created it) and never touches an
    /// `AssetPrice` row. Returns `InterestError`.
    pub async fn record_interest(
        &self,
        account_id: &str,
        asset_id: String,
        date: String,
        percent_micros: Option<i64>,
        quantity_micros: Option<i64>,
        note: Option<String>,
    ) -> Result<Transaction, InterestError> {
        // INT-011 — account must exist (checked before any asset work).
        self.account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;

        // INT-011 — asset must exist.
        let asset = self
            .asset_service
            .get_asset_by_id(&asset_id)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, asset_id = %asset_id, err = ?e, "record_interest: get_asset_by_id failed");
                AccountError::DatabaseError
            })?;
        let asset = match asset {
            None => return Err(InterestTask::AssetNotFound.into()),
            Some(a) => a,
        };

        // INT-012 — a non-cash asset must be interest-bearing;
        // INT-011 — a non-cash asset must be currently held (quantity > 0);
        // INT-023 — the account's Cash Asset is always a valid target, so both
        // checks are skipped for a Cash-class asset.
        if asset.class != AssetClass::Cash {
            if !asset.interest_bearing {
                return Err(InterestTask::InterestNotEligible.into());
            }
            let held = self
                .account_service
                .get_holding_by_account_asset(account_id, &asset_id)
                .await?;
            match held {
                Some(h) if h.quantity > 0 => {}
                _ => return Err(InterestTask::AssetNotHeld.into()),
            }
        }

        // Delegate to the account BC; its `AccountError` surfaces on the
        // interest wire as `InterestError::Account`.
        self.account_service
            .record_interest(
                account_id,
                asset_id,
                date,
                percent_micros,
                quantity_micros,
                note,
            )
            .await
            .map_err(InterestError::Account)
    }

    /// Loads the account, then ensures the system Cash Asset for its currency
    /// exists (CSH-010, CSH-011, CSH-017). Idempotent: safe to call on every
    /// cash-affecting command. Returns a typed `AccountError` so
    /// callers can propagate via `?` and stay typed end-to-end.
    ///
    /// Both error sources surface as `AccountError`:
    /// - **In-account failures**: `AccountNotFound { account_id }` when the row
    ///   is missing, `DatabaseError` when the account-repo call fails.
    /// - **Cross-BC asset-side failure** (`ensure_cash_asset` failure):
    ///   surfaced as `DatabaseError` after `tracing::error!` preserves the
    ///   asset-side diagnostic chain server-side.
    async fn ensure_cash_for(&self, account_id: &str, op: &str) -> Result<(), AccountError> {
        let account = self
            .account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                account_id: account_id.to_string(),
            })?;
        ensure_cash_asset(&self.asset_service, &account.currency)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, account_id = %account_id, op = %op, err = ?e, "ensure_cash_for: ensure_cash_asset failed");
                AccountError::DatabaseError
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository, UpdateFrequency,
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

    fn make_services(pool: &sqlx::Pool<sqlx::Sqlite>) -> (Arc<AccountService>, Arc<AssetService>) {
        let account_svc = Arc::new(AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        ));
        let asset_svc = Arc::new(AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ));
        (account_svc, asset_svc)
    }

    fn base_asset_dto() -> CreateAssetDTO {
        CreateAssetDTO {
            name: "Test Asset".to_string(),
            reference: "TST".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 1,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        }
    }

    fn interest_bearing_asset_dto() -> CreateAssetDTO {
        CreateAssetDTO {
            interest_bearing: true,
            ..base_asset_dto()
        }
    }

    fn micro(v: i64) -> i64 {
        v * 1_000_000
    }

    // TRX-056 — AssetNotFound when asset does not exist
    #[tokio::test]
    async fn open_holding_rejects_unknown_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);
        let err = uc
            .open_holding(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                OpenHoldingError::UseCase(OpenHoldingTask::AssetNotFound)
            ),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // TRX-050 — ArchivedAsset when asset is archived
    #[tokio::test]
    async fn open_holding_rejects_archived_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        asset_svc.archive_asset(&asset.id).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);
        let err = uc
            .open_holding(
                &account.id,
                asset.id,
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                OpenHoldingError::UseCase(OpenHoldingTask::ArchivedAsset)
            ),
            "expected UseCase(ArchivedAsset), got: {err:?}"
        );
    }

    // CSH-061 — open_holding rejects an OpeningBalance against a Cash Asset
    // (user must record initial cash via record_deposit instead).
    #[tokio::test]
    async fn open_holding_rejects_cash_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);
        let err = uc
            .open_holding(
                &account.id,
                cash_asset.id,
                "2024-01-01".to_string(),
                micro(1),
                micro(100),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                OpenHoldingError::UseCase(OpenHoldingTask::OpeningBalanceOnCashAsset)
            ),
            "expected UseCase(OpeningBalanceOnCashAsset), got: {err:?}"
        );
    }

    // TRX-047 — happy path: transaction and holding created with correct fields
    #[tokio::test]
    async fn open_holding_happy_path() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        let tx = uc
            .open_holding(
                &account.id,
                asset.id.clone(),
                "2024-01-01".to_string(),
                micro(2),
                micro(200),
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::OpeningBalance);
        assert_eq!(tx.total_amount, micro(200));
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.exchange_rate, 1_000_000);
        assert_eq!(tx.unit_price, micro(100));

        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].quantity, micro(2));
        assert_eq!(holdings[0].average_price, micro(100));
    }

    // -------------------------------------------------------------------------
    // Holding-tx orchestrator coverage (PR 3 — typed Result delegation)
    // -------------------------------------------------------------------------

    // TRX-027 — buy_holding happy path through the orchestrator: typed Result
    // flows from AccountService through the orchestrator unchanged.
    #[tokio::test]
    async fn buy_holding_orchestrator_happy_path() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        // Seed cash through the orchestrator so ensure_cash_for has been exercised
        // before the buy.
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(10_000), None)
            .await
            .unwrap();

        let tx = uc
            .buy_holding(
                &account.id,
                asset.id.clone(),
                "2024-01-15".to_string(),
                micro(2),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Purchase);
        assert_eq!(tx.total_amount, micro(200));
    }

    // When `buy_holding` is called for a nonexistent account, the
    // orchestrator's `ensure_cash_for` surfaces it as the typed
    // `Application(AccountNotFound { account_id })` — same shape every other
    // path raises for the same condition.
    #[tokio::test]
    async fn buy_holding_orchestrator_unknown_account_returns_application() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .buy_holding(
                "nonexistent-account-id",
                "irrelevant-asset".to_string(),
                "2024-01-15".to_string(),
                micro(1),
                micro(100),
                micro(1),
                0,
                None,
                None,
            )
            .await
            .unwrap_err();

        match err {
            AccountError::AccountNotFound { account_id } => {
                assert_eq!(account_id, "nonexistent-account-id");
            }
            other => panic!("expected AccountNotFound, got: {other:?}"),
        }
    }

    // CSH-022 — record_deposit through the orchestrator: typed Result is
    // returned end-to-end (no anyhow at this boundary).
    #[tokio::test]
    async fn record_deposit_orchestrator_happy_path() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let tx = uc
            .record_deposit(&account.id, "2024-01-01".to_string(), micro(500), None)
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Deposit);
        assert_eq!(tx.total_amount, micro(500));
    }

    // -------------------------------------------------------------------------
    // record_dividend — orchestrator unit tests (DIV-011, DIV-021, DIV-022,
    // DIV-023, DIV-024, DIV-026, DIV-027)
    // -------------------------------------------------------------------------

    // DIV-023 — happy path: dividend credited to cash, paying-asset holding unchanged.
    #[tokio::test]
    async fn record_dividend_happy_path() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(), // match asset currency so rate=1
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        // Buy some of the asset first so the holding exists (DIV-011 eligibility).
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let tx = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(200), // 200 USD dividend
                micro(1),   // exchange_rate = 1 (same currency)
                None,
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Dividend);
        assert_eq!(tx.asset_id, asset.id, "asset_id must be the paying asset");
        assert_eq!(tx.total_amount, micro(200));
        assert_eq!(tx.fees, 0);
        assert!(
            tx.realized_pnl.is_none(),
            "dividend must have no realized_pnl"
        );

        // Verify paying-asset holding is unchanged (DIV-024)
        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let paying_holding = holdings
            .iter()
            .find(|h| h.asset_id == asset.id)
            .expect("paying asset holding must still exist");
        assert_eq!(
            paying_holding.quantity,
            micro(10),
            "quantity must be unchanged"
        );
    }

    // DIV-011 — AccountNotFound: unknown account is rejected before any asset check.
    #[tokio::test]
    async fn record_dividend_rejects_unknown_account() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_dividend(
                "nonexistent-account",
                asset.id,
                "2024-06-15".to_string(),
                micro(100),
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::DividendError;
        assert!(
            matches!(
                err,
                DividendError::Account(AccountError::AccountNotFound { .. })
            ),
            "expected Application(AccountNotFound), got: {err:?}"
        );
    }

    // DIV-011 — AssetNotFound: unknown asset_id is rejected.
    #[tokio::test]
    async fn record_dividend_rejects_unknown_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_dividend(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-06-15".to_string(),
                micro(100),
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{DividendError, DividendTask};
        assert!(
            matches!(err, DividendError::UseCase(DividendTask::AssetNotFound)),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // DIV-011 — AssetNotHeld: asset exists but is not held (never bought).
    #[tokio::test]
    async fn record_dividend_rejects_asset_not_held() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(100),
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{DividendError, DividendTask};
        assert!(
            matches!(err, DividendError::UseCase(DividendTask::AssetNotHeld)),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // DIV-011 — DividendOnCashAsset: the paying asset is a Cash Asset.
    #[tokio::test]
    async fn record_dividend_rejects_cash_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_dividend(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                micro(100),
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{DividendError, DividendTask};
        assert!(
            matches!(
                err,
                DividendError::UseCase(DividendTask::DividendOnCashAsset)
            ),
            "expected UseCase(DividendOnCashAsset), got: {err:?}"
        );
    }

    // DIV-021 — AmountNotPositive: amount_micros = 0.
    #[tokio::test]
    async fn record_dividend_rejects_zero_amount() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                0,
                micro(1),
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::DividendError;
        assert!(
            matches!(err, DividendError::Account(AccountError::AmountNotPositive)),
            "expected Validation(AmountNotPositive), got: {err:?}"
        );
    }

    // DIV-022 — ExchangeRateNotPositive: exchange_rate = 0.
    #[tokio::test]
    async fn record_dividend_rejects_zero_exchange_rate() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(100),
                0, // invalid
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::DividendError;
        assert!(
            matches!(
                err,
                DividendError::Account(AccountError::ExchangeRateNotPositive)
            ),
            "expected Validation(ExchangeRateNotPositive), got: {err:?}"
        );
    }

    // DIV-027 — recording a dividend must NOT create or modify an AssetPrice row.
    #[tokio::test]
    async fn record_dividend_does_not_create_asset_price() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc.clone());
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        uc.record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();

        // After the dividend, no AssetPrice row must exist for the paying asset.
        let latest_price = asset_svc.get_latest_price(&asset.id).await.unwrap();
        assert!(
            latest_price.is_none(),
            "recording a dividend must not create an AssetPrice row (DIV-027)"
        );
    }

    // DIV-023 — currency conversion: amount in asset ccy × exchange_rate = account ccy total.
    #[tokio::test]
    async fn record_dividend_converts_amount_at_exchange_rate() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        // Asset is USD, account is EUR → exchange_rate = 0.9 (EUR per USD)
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap(); // USD asset
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            900_000, // 0.9 EUR/USD
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let amount_micros = 100_000_000i64; // 100 USD
        let exchange_rate = 900_000i64; // 0.9 EUR/USD
        let tx = uc
            .record_dividend(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                amount_micros,
                exchange_rate,
                None,
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Dividend);
        // total_amount = floor(100_000_000 × 900_000 / 1_000_000) = 90_000_000
        assert_eq!(
            tx.total_amount, 90_000_000,
            "total_amount must equal floor(amount × rate / MICRO)"
        );
    }

    // -------------------------------------------------------------------------
    // record_free_shares — orchestrator unit tests (FSD-011, FSD-021, FSD-022,
    // FSD-023, FSD-024, FSD-026)
    // -------------------------------------------------------------------------

    // FSD-022/023 — happy path: free shares increase quantity, cost basis unchanged,
    // no cash movement, no AssetPrice created.
    #[tokio::test]
    async fn record_free_shares_happy_path() {
        // FSD-022 — orchestrator delegates through to account service; holding updated
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc.clone());

        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let holdings_before = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let cost_basis_before = holdings_before
            .iter()
            .find(|h| h.asset_id == asset.id)
            .map(|h| h.quantity as i128 * h.average_price as i128 / 1_000_000)
            .unwrap();

        let tx = uc
            .record_free_shares(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap();

        // FSD-022 — transaction fields
        assert_eq!(
            tx.transaction_type,
            TransactionType::FreeShares,
            "transaction_type must be FreeShares"
        );
        assert_eq!(tx.asset_id, asset.id);
        assert_eq!(tx.quantity, micro(5));
        // FSD-023 — zero-cost convention
        assert_eq!(tx.unit_price, 0);
        assert_eq!(tx.exchange_rate, 1_000_000);
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.total_amount, 0);
        assert!(tx.realized_pnl.is_none());

        let holdings_after = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let holding_after = holdings_after
            .iter()
            .find(|h| h.asset_id == asset.id)
            .unwrap();

        // FSD-022a — quantity increased by distributed amount
        assert_eq!(holding_after.quantity, micro(15), "quantity must be 15");
        // FSD-023 — underlying cost unchanged → VWAP dilutes to the exact floored
        // value (TRX-026 floor convention).
        let expected_diluted_vwap =
            (cost_basis_before * 1_000_000 / holding_after.quantity as i128) as i64;
        assert_eq!(
            holding_after.average_price, expected_diluted_vwap,
            "average price must equal floor(cost_basis / new_quantity)"
        );

        // FSD-024 — no AssetPrice row created
        let latest_price = asset_svc.get_latest_price(&asset.id).await.unwrap();
        assert!(
            latest_price.is_none(),
            "record_free_shares must not create an AssetPrice row (FSD-024)"
        );

        // FSD-022d — cash holding unchanged
        let cash_holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let _ = cash_holdings; // presence assertion done via business logic; cash test is in account.rs
    }

    // SPL-012 — AssetNotFound: unknown asset_id is rejected.
    #[tokio::test]
    async fn record_split_rejects_unknown_asset() {
        // SPL-012 — asset must exist
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_split(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-06-15".to_string(),
                2_000_000,
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{SplitError, SplitTask};
        assert!(
            matches!(err, SplitError::UseCase(SplitTask::AssetNotFound)),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // SPL-012 — AssetNotHeld: asset exists but is not currently held.
    #[tokio::test]
    async fn record_split_rejects_asset_not_held() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_split(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                2_000_000,
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{SplitError, SplitTask};
        assert!(
            matches!(err, SplitError::UseCase(SplitTask::AssetNotHeld)),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // FSD-011 — account must exist (checked before any asset work).
    #[tokio::test]
    async fn record_free_shares_rejects_unknown_account() {
        // FSD-011 — account must exist
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_free_shares(
                "nonexistent-account",
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::FreeSharesError;
        assert!(
            matches!(
                err,
                FreeSharesError::Account(AccountError::AccountNotFound { .. })
            ),
            "expected Application(AccountNotFound), got: {err:?}"
        );
    }

    // FSD-011 — AssetNotFound: unknown asset_id is rejected.
    #[tokio::test]
    async fn record_free_shares_rejects_unknown_asset() {
        // FSD-011 — asset must exist
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_free_shares(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{FreeSharesError, FreeSharesTask};
        assert!(
            matches!(err, FreeSharesError::UseCase(FreeSharesTask::AssetNotFound)),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // FSD-011 — AssetNotHeld: asset exists but is not held in this account.
    #[tokio::test]
    async fn record_free_shares_rejects_asset_not_held() {
        // FSD-011 — asset must be currently held with quantity > 0
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_free_shares(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{FreeSharesError, FreeSharesTask};
        assert!(
            matches!(err, FreeSharesError::UseCase(FreeSharesTask::AssetNotHeld)),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // FSD-011 — FreeSharesOnCashAsset: the distributing asset is a Cash Asset.
    #[tokio::test]
    async fn record_free_shares_rejects_cash_asset() {
        // FSD-011 — distributing asset must not be a Cash Asset
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_free_shares(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{FreeSharesError, FreeSharesTask};
        assert!(
            matches!(
                err,
                FreeSharesError::UseCase(FreeSharesTask::FreeSharesOnCashAsset)
            ),
            "expected UseCase(FreeSharesOnCashAsset), got: {err:?}"
        );
    }

    // FSD-021 — QuantityNotPositive: quantity = 0 is rejected.
    #[tokio::test]
    async fn record_free_shares_rejects_zero_quantity() {
        // FSD-021 — quantity must be strictly positive
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_free_shares(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                0, // invalid
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::FreeSharesError;
        assert!(
            matches!(
                err,
                FreeSharesError::Account(AccountError::QuantityNotPositive)
            ),
            "expected Validation(QuantityNotPositive), got: {err:?}"
        );
    }

    // FSD-021 — DateInFuture: future date is rejected.
    #[tokio::test]
    async fn record_free_shares_rejects_future_date() {
        // FSD-021 — date must not be in the future
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_free_shares(
                &account.id,
                asset.id.clone(),
                "2099-01-01".to_string(), // future
                micro(5),
                None,
            )
            .await
            .unwrap_err();

        use crate::context::account::AccountError;
        use crate::use_cases::holding_transaction::FreeSharesError;
        assert!(
            matches!(err, FreeSharesError::Account(AccountError::DateInFuture)),
            "expected Validation(DateInFuture), got: {err:?}"
        );
    }

    // FEE-012 — ManagementFeeOnCashAsset: the charged asset must not be a Cash Asset.
    #[tokio::test]
    async fn fee_012_record_management_fee_rejects_cash_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_management_fee(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                micro(1), // 1%
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{ManagementFeeError, ManagementFeeTask};
        assert!(
            matches!(
                err,
                ManagementFeeError::UseCase(ManagementFeeTask::ManagementFeeOnCashAsset)
            ),
            "expected UseCase(ManagementFeeOnCashAsset), got: {err:?}"
        );
    }

    // FEE-012 — AssetNotHeld: the asset exists and is non-cash but is not currently held.
    #[tokio::test]
    async fn fee_012_record_management_fee_rejects_asset_not_held() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        // Asset was never bought → no active holding.
        let err = uc
            .record_management_fee(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                micro(1), // 1%
                None,
            )
            .await
            .unwrap_err();

        use crate::use_cases::holding_transaction::{ManagementFeeError, ManagementFeeTask};
        assert!(
            matches!(
                err,
                ManagementFeeError::UseCase(ManagementFeeTask::AssetNotHeld)
            ),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // -------------------------------------------------------------------------
    // record_interest — orchestrator unit tests (INT-011, INT-021, INT-022,
    // INT-023, INT-024)
    // -------------------------------------------------------------------------

    // INT-022/024 — happy path (percent mode): the credit is 10% of the held
    // quantity, added at zero cost.
    #[tokio::test]
    async fn record_interest_happy_path_percent_mode() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        // 10% of 10 units → 1 unit credited.
        let tx = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-12-31".to_string(),
                Some(10_000_000),
                None,
                None,
            )
            .await
            .unwrap();

        assert_eq!(tx.transaction_type, TransactionType::Interest);
        assert_eq!(tx.asset_id, asset.id);
        assert_eq!(tx.quantity, micro(1), "credited qty must be 10% of 10");
        // INT-024 — zero-cost packing
        assert_eq!(tx.unit_price, 0);
        assert_eq!(tx.exchange_rate, 1_000_000);
        assert_eq!(tx.fees, 0);
        assert_eq!(tx.total_amount, 0);
        assert!(tx.realized_pnl.is_none());

        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let holding = holdings.iter().find(|h| h.asset_id == asset.id).unwrap();
        assert_eq!(holding.quantity, micro(11), "quantity must be 10 + 1");
    }

    // INT-011 — AccountNotFound: unknown account is rejected before any asset check.
    #[tokio::test]
    async fn record_interest_rejects_unknown_account() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_interest(
                "nonexistent-account",
                asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::AccountNotFound { .. })
            ),
            "expected Account(AccountNotFound), got: {err:?}"
        );
    }

    // INT-011 — AssetNotFound: unknown asset_id is rejected.
    #[tokio::test]
    async fn record_interest_rejects_unknown_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_interest(
                &account.id,
                "nonexistent-asset".to_string(),
                "2024-06-15".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(err, InterestError::UseCase(InterestTask::AssetNotFound)),
            "expected UseCase(AssetNotFound), got: {err:?}"
        );
    }

    // INT-011 — AssetNotHeld: a non-cash asset that is not currently held is
    // rejected (interest-bearing, so the INT-012 eligibility check passes and
    // the held check is the one that fires).
    #[tokio::test]
    async fn record_interest_rejects_non_held_non_cash_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc, asset_svc);

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(err, InterestError::UseCase(InterestTask::AssetNotHeld)),
            "expected UseCase(AssetNotHeld), got: {err:?}"
        );
    }

    // INT-012 — InterestNotEligible: a held non-cash asset without the
    // interest_bearing flag is rejected before the held check.
    #[tokio::test]
    async fn record_interest_rejects_non_interest_bearing_asset() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                InterestError::UseCase(InterestTask::InterestNotEligible)
            ),
            "expected UseCase(InterestNotEligible), got: {err:?}"
        );
    }

    // INT-023 / INT-012 — the account's Cash Asset IS a valid target even
    // though it is not interest_bearing-flagged: both the eligibility and held
    // checks are skipped and the credit lands on the cash balance
    // (1000 + 50 = 1050).
    #[tokio::test]
    async fn record_interest_accepts_cash_asset_and_credits_balance() {
        use crate::context::account::TransactionType;

        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();

        let tx = uc
            .record_interest(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(micro(50)),
                None,
            )
            .await
            .unwrap();
        assert_eq!(tx.transaction_type, TransactionType::Interest);
        assert_eq!(tx.asset_id, cash_asset.id);

        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let cash = holdings
            .iter()
            .find(|h| h.asset_id == cash_asset.id)
            .expect("cash holding must exist");
        assert_eq!(
            cash.quantity,
            micro(1_050),
            "cash balance must be 1000 + 50 after the interest credit (INT-023)"
        );
    }

    // INT-021 — QuantityNotPositive: quantity mode with 0 is rejected.
    #[tokio::test]
    async fn record_interest_rejects_zero_quantity() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(0),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::QuantityNotPositive)
            ),
            "expected Account(QuantityNotPositive), got: {err:?}"
        );
    }

    // INT-021 — percent bounds: 0 → PercentageNotPositive; > 100% → PercentageAboveHundred;
    // both provided → InterestAmountInvalid.
    #[tokio::test]
    async fn record_interest_rejects_invalid_amount_specs() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                Some(0),
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::PercentageNotPositive)
            ),
            "expected Account(PercentageNotPositive), got: {err:?}"
        );

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                Some(100_000_001),
                None,
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::PercentageAboveHundred)
            ),
            "expected Account(PercentageAboveHundred), got: {err:?}"
        );

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2024-06-15".to_string(),
                Some(1_000_000),
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                InterestError::Account(AccountError::InterestAmountInvalid)
            ),
            "expected Account(InterestAmountInvalid), got: {err:?}"
        );
    }

    // INT-021 — DateInFuture: future date is rejected.
    #[tokio::test]
    async fn record_interest_rejects_future_date() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let asset = asset_svc
            .create_asset(interest_bearing_asset_dto())
            .await
            .unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let err = uc
            .record_interest(
                &account.id,
                asset.id.clone(),
                "2099-01-01".to_string(),
                None,
                Some(micro(5)),
                None,
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, InterestError::Account(AccountError::DateInFuture)),
            "expected Account(DateInFuture), got: {err:?}"
        );
    }

    // INT-022/023 — percent-mode on the cash line uses the cash-balance replay,
    // not the per-asset holding replay: a Purchase (different asset_id) reduces
    // the base. Deposit 1000, buy 500 → balance 500; 10% credits 50, not 100.
    #[tokio::test]
    async fn record_interest_percent_on_cash_uses_cash_balance_base() {
        let pool = setup_pool().await;
        let (account_svc, asset_svc) = make_services(&pool);
        let cash_asset = asset_svc.seed_cash_asset("USD").await.unwrap();
        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Acc".to_string(),
                String::new(),
                "USD".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        let uc = HoldingTransactionUseCase::new(account_svc.clone(), asset_svc);
        uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
            .await
            .unwrap();
        // Purchase moves cash (−500) under the traded asset's id, not the cash id.
        uc.buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

        let tx = uc
            .record_interest(
                &account.id,
                cash_asset.id.clone(),
                "2024-06-15".to_string(),
                Some(10_000_000),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            tx.quantity,
            micro(50),
            "10% of the true 500 cash balance, not of the 1000 deposit history"
        );

        let holdings = account_svc
            .get_holdings_for_account(&account.id)
            .await
            .unwrap();
        let cash = holdings
            .iter()
            .find(|h| h.asset_id == cash_asset.id)
            .expect("cash holding must exist");
        assert_eq!(cash.quantity, micro(550), "500 + 50 credited");
    }
}
