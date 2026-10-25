use crate::context::account::{Account, AccountError, AccountServiceContract, AgentRecording};
use crate::context::asset::AssetServiceContract;
use crate::core::logger::BACKEND;
use crate::shared::infrastructure::uow::TransactionManager;
use std::result::Result as StdResult;
use std::sync::Arc;

/// Orchestrates account creation across the account and asset bounded contexts
/// (ACC-025). Injects `AccountService` + `AssetService` per B18 / B24 —
/// no `account` → `asset` import is introduced.
pub struct AccountCreationUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    transactions: Arc<dyn TransactionManager>,
}

impl AccountCreationUseCase {
    /// Creates a new `AccountCreationUseCase`.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        transactions: Arc<dyn TransactionManager>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            transactions,
        }
    }

    /// Creates an account and seeds its 0-balance Cash Holding (ACC-025, CSH-010 / CSH-012).
    pub async fn create(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
    ) -> StdResult<Account, AccountError> {
        self.create_classified(
            name,
            bank_name,
            currency,
            management_fees_enabled,
            None,
            None,
        )
        .await
    }

    /// The account with its country and wrapper (ACC-041), seeded as `create` seeds it.
    // The account's own fields, one argument each as `create`/`update` take them, plus
    // its country and wrapper (ACC-041).
    #[allow(clippy::too_many_arguments)]
    pub async fn create_classified(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        management_fees_enabled: bool,
        country: Option<String>,
        wrapper: Option<String>,
    ) -> StdResult<Account, AccountError> {
        // ACC-002 / TRX-021 / ACC-041 — the account's own fields are checked before anything
        // is seeded for it: a value it refuses is answered with its own code.
        let account = Account::new(name, bank_name, currency, management_fees_enabled)?
            .classified(country, wrapper, None)?;
        self.create_with_cash(account, None).await
    }

    /// AGT-068 — creates the account an agent asked for as the window creates one, its cash
    /// line included (ACC-025), marked with the agent and its session (AGT-069).
    pub async fn create_by_agent(
        &self,
        name: String,
        bank_name: String,
        currency: String,
        country: Option<String>,
        wrapper: Option<String>,
        mark: AgentRecording,
    ) -> StdResult<Account, AccountError> {
        let account =
            Account::new(name, bank_name, currency, false)?.classified(country, wrapper, None)?;
        self.create_with_cash(account, Some(mark)).await
    }

    /// ACC-025 — the Cash Asset of the account's currency (CSH-010), the account and its
    /// cash line, in one unit of work (ADR-006): all of them or none.
    async fn create_with_cash(
        &self,
        account: Account,
        mark: Option<AgentRecording>,
    ) -> StdResult<Account, AccountError> {
        let currency = account.currency.clone();
        let mut unit = self.transactions.begin().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "create_account: unit of work not begun");
            AccountError::DatabaseError
        })?;
        let cash_asset_created = self
            .asset_service
            .seed_cash_asset_in(&mut unit, &currency)
            .await
            .map_err(|e| {
                tracing::error!(target: BACKEND, currency = %currency, err = ?e, "create_account: seed_cash_asset failed");
                AccountError::DatabaseError
            })?;
        let account = self
            .account_service
            .create_in(&mut unit, account, mark)
            .await?;
        unit.commit().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "create_account: unit of work not committed");
            AccountError::DatabaseError
        })?;
        if cash_asset_created {
            self.asset_service.notify_asset_updated();
        }
        self.account_service.notify_account_updated();
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
    use crate::shared::infrastructure::uow::SqliteTransactionManager;
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
        AccountCreationUseCase::new(
            account_svc,
            asset_svc,
            Arc::new(SqliteTransactionManager::new(pool.clone())),
        )
    }

    // TRX-021 — a currency that is none is refused with its own code, and nothing is
    // seeded for the account that was not created.
    #[tokio::test]
    async fn trx_021_an_invalid_currency_is_refused_before_anything_is_seeded() {
        let pool = setup_pool().await;
        let uc = make_uc(&pool);

        let refused = uc
            .create(
                "Brokerage".to_string(),
                String::new(),
                "EURO".to_string(),
                false,
            )
            .await
            .expect_err("refused");

        assert!(
            matches!(&refused, AccountError::InvalidCurrency { currency } if currency == "EURO"),
            "{refused:?}"
        );
        let seeded: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM assets")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(seeded.0, 0, "no Cash Asset is seeded");
    }

    // ACC-025 / ADR-006 — the Cash Asset, the account and its cash line are one unit of work:
    // when the cash line cannot be written, nothing is, and the same account can be created
    // again instead of being refused as a duplicate.
    #[tokio::test]
    async fn acc_025_a_failed_cash_line_leaves_nothing_and_a_retry_succeeds() {
        let pool = setup_pool().await;
        let uc = make_uc(&pool);
        sqlx::query(
            "CREATE TRIGGER refuse_holdings BEFORE INSERT ON holdings
             BEGIN SELECT RAISE(ABORT, 'refused'); END",
        )
        .execute(&pool)
        .await
        .expect("trigger");

        let failed = uc
            .create(
                "Brokerage".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await;

        assert!(
            matches!(failed, Err(AccountError::DatabaseError)),
            "{failed:?}"
        );
        let count = |table: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                    .fetch_one(&pool)
                    .await
                    .expect("count")
            }
        };
        assert_eq!(count("accounts").await, 0, "no account is left");
        assert_eq!(count("assets").await, 0, "no Cash Asset is left");

        sqlx::query("DROP TRIGGER refuse_holdings")
            .execute(&pool)
            .await
            .expect("drop trigger");
        uc.create(
            "Brokerage".to_string(),
            String::new(),
            "EUR".to_string(),
            false,
        )
        .await
        .expect("the retry creates the account");
        assert_eq!(count("holdings").await, 1, "with its cash line");
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
            .create("Plain".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        assert!(!disabled.management_fees_enabled);
        let enabled = uc
            .create("Funds".to_string(), String::new(), "EUR".to_string(), true)
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

        uc.create("A".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        uc.create("B".to_string(), String::new(), "EUR".to_string(), false)
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
