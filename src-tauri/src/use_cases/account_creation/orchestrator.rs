use crate::context::account::{Account, AccountError, AccountServiceContract, UpdateFrequency};
use crate::context::asset::AssetServiceContract;
use crate::core::logger::BACKEND;
use std::result::Result as StdResult;
use std::sync::Arc;

/// Orchestrates account creation across the account and asset bounded contexts
/// (ACC-025). Injects `AccountService` + `AssetService` per ADR-003 / ADR-004 —
/// no `account` → `asset` import is introduced.
pub struct AccountCreationUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
}

impl AccountCreationUseCase {
    /// Creates a new `AccountCreationUseCase`.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
        }
    }

    /// Creates an account and eagerly seeds its 0-balance Cash Holding
    /// (ACC-025, CSH-010 / CSH-012).
    ///
    /// Three sequential commits, not a single Unit of Work: ensure Cash Asset →
    /// create account → seed Cash Holding. This mirrors the existing non-atomic
    /// ensure-then-write cash pattern in `holding_transaction`; a mid-sequence
    /// failure leaves a self-healing state (the backfill migration or a re-run
    /// repairs an account left without its Cash Holding).
    pub async fn create(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        update_frequency: UpdateFrequency,
        management_fees_enabled: bool,
    ) -> StdResult<Account, AccountError> {
        // CSH-010 — the Cash Asset must exist before the Cash Holding references it (FK).
        self.asset_service
            .seed_cash_asset(&currency)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, currency = %currency, err = ?e, "create_account: seed_cash_asset failed");
                AccountError::DatabaseError
            })?;
        // Account row — single-write create path (enforces ACC-001 / ACC-002 /
        // ACC-003; FEE-075's disabled default comes from the creation form).
        let account = self
            .account_service
            .create(
                name,
                bank_name,
                currency,
                update_frequency,
                management_fees_enabled,
            )
            .await?;
        // CSH-012 — eager 0-balance Cash Holding.
        self.account_service.seed_cash_holding(&account.id).await?;
        Ok(account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository,
    };
    use crate::context::asset::{
        AssetService, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
        SqliteAssetRepository,
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

    fn make_uc(pool: &sqlx::Pool<sqlx::Sqlite>) -> AccountCreationUseCase {
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
        AccountCreationUseCase::new(account_svc, asset_svc)
    }

    // ACC-025 / CSH-010 / CSH-012 — create seeds the Cash Asset and a 0-balance Cash Holding.
    #[tokio::test]
    async fn create_seeds_cash_asset_and_zero_balance_holding() {
        let pool = setup_pool().await;
        let uc = make_uc(&pool);

        let account = uc
            .create(
                "Brokerage".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .expect("create");

        let asset_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM assets WHERE id = 'system-cash-eur'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            asset_count.0, 1,
            "the per-currency Cash Asset is seeded (CSH-010)"
        );

        let (quantity, average_price): (i64, i64) = sqlx::query_as(
            "SELECT quantity, average_price FROM holdings WHERE account_id = ? AND asset_id = 'system-cash-eur'",
        )
        .bind(&account.id)
        .fetch_one(&pool)
        .await
        .expect("a cash holding row must exist for the new account (CSH-012)");
        assert_eq!(quantity, 0);
        assert_eq!(average_price, 1_000_000);
    }

    // FEE-075 — creation opt-in flips the flag on; the default stays off.
    #[tokio::test]
    async fn create_with_management_fees_opt_in_enables_the_flag() {
        let pool = setup_pool().await;
        let uc = make_uc(&pool);
        let disabled = uc
            .create(
                "Plain".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        assert!(!disabled.management_fees_enabled);
        let enabled = uc
            .create(
                "Funds".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                true,
            )
            .await
            .unwrap();
        assert!(enabled.management_fees_enabled);
    }

    // CSH-011 — a second account in the same currency reuses the single Cash Asset,
    // each account getting its own Cash Holding.
    #[tokio::test]
    async fn second_account_same_currency_reuses_cash_asset() {
        let pool = setup_pool().await;
        let uc = make_uc(&pool);

        uc.create(
            "A".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
        uc.create(
            "B".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

        let asset_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM assets WHERE id = 'system-cash-eur'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(asset_count.0, 1, "a single shared Cash Asset per currency");

        let holding_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM holdings WHERE asset_id = 'system-cash-eur'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(holding_count.0, 2, "one Cash Holding per account");
    }
}
