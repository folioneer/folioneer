use crate::context::account::{AccountError, AccountServiceContract};
use serde::Serialize;
use specta::Type;
use std::result::Result as StdResult;
use std::sync::Arc;

/// Pre-deletion counts for an account (ACC-020).
#[derive(Debug, Serialize, Type)]
pub struct AccountDeletionSummary {
    /// Number of active holdings (quantity > 0) in the account.
    pub holding_count: u32,
    /// Total number of transactions associated with the account.
    pub transaction_count: u32,
}

/// Reads the deletion summary for an account without mutating any state (ACC-020).
///
/// Injects only AccountService because after Phase 7 both holdings and
/// transactions live within the account bounded context.
pub struct AccountDeletionUseCase {
    account_service: Arc<dyn AccountServiceContract>,
}

impl AccountDeletionUseCase {
    /// Creates a new AccountDeletionUseCase.
    pub fn new(account_service: Arc<dyn AccountServiceContract>) -> Self {
        Self { account_service }
    }

    /// Returns holding and transaction counts for the given account (ACC-020).
    pub async fn get_summary(
        &self,
        account_id: &str,
    ) -> StdResult<AccountDeletionSummary, AccountError> {
        let (holding_count, transaction_count) = self
            .account_service
            .get_deletion_summary(account_id)
            .await?;
        Ok(AccountDeletionSummary {
            holding_count,
            transaction_count,
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

    fn make_account_service(pool: &sqlx::Pool<sqlx::Sqlite>) -> Arc<AccountService> {
        Arc::new(AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        ))
    }

    fn make_asset_service(pool: &sqlx::Pool<sqlx::Sqlite>) -> Arc<AssetService> {
        Arc::new(AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ))
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

    // ACC-020 — empty account returns zero counts
    #[tokio::test]
    async fn test_get_summary_empty_account_returns_zero_counts() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let account = account_svc
            .create(
                "Empty Account".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let uc = AccountDeletionUseCase::new(account_svc);
        let summary = uc.get_summary(&account.id).await.unwrap();

        assert_eq!(summary.holding_count, 0);
        assert_eq!(summary.transaction_count, 0);
    }

    // ACC-020 — account with one buy returns holding_count=1, transaction_count=1
    #[tokio::test]
    async fn test_get_summary_one_buy_returns_one_holding_one_transaction() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);

        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Test Account".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        // Cash is now a Holding (CSH-090): seed it before any purchase so CSH-041 holds.
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        account_svc
            .record_deposit(
                &account.id,
                "2020-01-01".to_string(),
                1_000_000_000_000,
                None,
            )
            .await
            .unwrap();
        account_svc
            .buy_holding(
                &account.id,
                asset.id.clone(),
                "2026-01-01".to_string(),
                1_000_000,
                10_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();

        let uc = AccountDeletionUseCase::new(account_svc);
        let summary = uc.get_summary(&account.id).await.unwrap();

        // 2 holdings (asset + cash), 2 transactions (Purchase + Deposit) — Cash is a Holding too.
        assert_eq!(summary.holding_count, 2);
        assert_eq!(summary.transaction_count, 2);
    }

    // ACC-020 — closed holding (qty=0) does not count as active
    #[tokio::test]
    async fn test_get_summary_closed_holding_not_counted_as_active() {
        let pool = setup_pool().await;
        let account_svc = make_account_service(&pool);
        let asset_svc = make_asset_service(&pool);

        let asset = asset_svc.create_asset(base_asset_dto()).await.unwrap();
        let account = account_svc
            .create(
                "Test Account".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        // Cash is now a Holding (CSH-090): seed it before any purchase so CSH-041 holds.
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        account_svc
            .record_deposit(
                &account.id,
                "2020-01-01".to_string(),
                1_000_000_000_000,
                None,
            )
            .await
            .unwrap();
        account_svc
            .buy_holding(
                &account.id,
                asset.id.clone(),
                "2026-01-01".to_string(),
                1_000_000,
                10_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .sell_holding(
                &account.id,
                asset.id.clone(),
                "2026-01-02".to_string(),
                1_000_000,
                12_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();

        let uc = AccountDeletionUseCase::new(account_svc);
        let summary = uc.get_summary(&account.id).await.unwrap();

        // Asset holding closed (qty=0) — Cash Holding still active.
        assert_eq!(summary.holding_count, 1);
        // 3 transactions: Deposit + Purchase + Sell.
        assert_eq!(summary.transaction_count, 3);
    }

    // get_summary translates raw repo failure to AccountError::DatabaseError.
    #[tokio::test]
    async fn get_summary_translates_repo_failure_to_database_error() {
        use crate::context::account::{
            MockAccountRepository, MockHoldingRepository, MockTransactionRepository,
        };

        let mut mock_hr = MockHoldingRepository::new();
        mock_hr
            .expect_count_active_for_account()
            .returning(|_| Err(anyhow::anyhow!("simulated holding-repo failure")));
        let mut mock_tr = MockTransactionRepository::new();
        mock_tr
            .expect_count_by_account()
            .returning(|_| Err(anyhow::anyhow!("simulated transaction-repo failure")));
        let svc = Arc::new(AccountService::new(
            Box::new(MockAccountRepository::new()),
            Box::new(mock_hr),
            Box::new(mock_tr),
        ));
        let uc = AccountDeletionUseCase::new(svc);
        let err = uc.get_summary("any-id").await.unwrap_err();
        assert!(matches!(err, AccountError::DatabaseError), "got: {err:?}");
    }
}
