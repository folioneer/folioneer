use super::error::{DeleteAssetError, DeleteAssetTask};
use crate::context::account::AccountServiceContract;
use crate::context::asset::AssetServiceContract;
use std::result::Result as StdResult;
use std::sync::Arc;

/// Guards and delegates asset hard-deletion across the asset and account bounded contexts.
/// Blocks deletion if any transaction references the asset (preserves history integrity).
pub struct DeleteAssetUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
}

impl DeleteAssetUseCase {
    /// Creates a new DeleteAssetUseCase.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
        }
    }

    /// Deletes an asset, rejecting the request if any transaction references it.
    pub async fn delete_asset(&self, asset_id: &str) -> StdResult<(), DeleteAssetError> {
        if self
            .account_service
            .has_holding_entries_for_asset(asset_id)
            .await?
        {
            return Err(DeleteAssetTask::ExistingTransactions.into());
        }
        self.asset_service.delete_asset(asset_id).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::MockAccountServiceContract;
    use crate::context::asset::MockAssetServiceContract;
    use mockall::predicate::eq;

    // delete blocked when transactions exist; the asset BC is never reached.
    #[tokio::test]
    async fn delete_rejected_when_transactions_exist() {
        let mut account_svc = MockAccountServiceContract::new();
        account_svc
            .expect_has_holding_entries_for_asset()
            .once()
            .with(eq("asset-1"))
            .returning(|_| Ok(true));
        let asset_svc = MockAssetServiceContract::new();

        let uc = DeleteAssetUseCase::new(Arc::new(account_svc), Arc::new(asset_svc));
        let err = uc.delete_asset("asset-1").await.unwrap_err();
        assert!(
            matches!(
                err,
                DeleteAssetError::Application(DeleteAssetTask::ExistingTransactions)
            ),
            "got: {err:?}"
        );
    }

    // delete succeeds when no transactions exist
    #[tokio::test]
    async fn delete_succeeds_when_no_transactions() {
        let mut account_svc = MockAccountServiceContract::new();
        account_svc
            .expect_has_holding_entries_for_asset()
            .once()
            .with(eq("asset-1"))
            .returning(|_| Ok(false));
        let mut asset_svc = MockAssetServiceContract::new();
        asset_svc
            .expect_delete_asset()
            .once()
            .with(eq("asset-1"))
            .returning(|_| Ok(()));

        let uc = DeleteAssetUseCase::new(Arc::new(account_svc), Arc::new(asset_svc));
        uc.delete_asset("asset-1").await.unwrap();
    }

    // Cross-BC guard failure propagates as the account arm of the composite.
    #[tokio::test]
    async fn delete_propagates_account_guard_failure() {
        use crate::context::account::AccountError;
        let mut account_svc = MockAccountServiceContract::new();
        account_svc
            .expect_has_holding_entries_for_asset()
            .once()
            .returning(|_| Err(AccountError::DatabaseError));
        let asset_svc = MockAssetServiceContract::new();

        let uc = DeleteAssetUseCase::new(Arc::new(account_svc), Arc::new(asset_svc));
        let err = uc.delete_asset("asset-1").await.unwrap_err();
        assert!(
            matches!(err, DeleteAssetError::Account(AccountError::DatabaseError)),
            "got: {err:?}"
        );
    }
}
