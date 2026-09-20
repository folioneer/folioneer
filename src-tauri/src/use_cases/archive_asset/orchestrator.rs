use super::error::{ArchiveAssetError, ArchiveAssetTask};
use crate::context::account::AccountServiceContract;
use crate::context::asset::AssetServiceContract;
use std::result::Result as StdResult;
use std::sync::Arc;

/// Guards and delegates asset archiving across the asset and account bounded contexts (OQ-6).
pub struct ArchiveAssetUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
}

impl ArchiveAssetUseCase {
    /// Creates a new ArchiveAssetUseCase.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
        }
    }

    /// Archives an asset, rejecting the request if any account holds an active position (OQ-6).
    pub async fn archive_asset(&self, asset_id: &str) -> StdResult<(), ArchiveAssetError> {
        if self
            .account_service
            .has_active_holdings_for_asset(asset_id)
            .await?
        {
            return Err(ArchiveAssetTask::ActiveHoldings.into());
        }
        self.asset_service.archive_asset(asset_id).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::MockAccountServiceContract;
    use crate::context::asset::MockAssetServiceContract;
    use mockall::predicate::eq;

    // OQ-6 — archive rejected when active holding exists; the asset BC is never reached.
    #[tokio::test]
    async fn archive_rejected_when_active_holdings() {
        let mut account_svc = MockAccountServiceContract::new();
        account_svc
            .expect_has_active_holdings_for_asset()
            .once()
            .with(eq("asset-1"))
            .returning(|_| Ok(true));
        let asset_svc = MockAssetServiceContract::new();

        let uc = ArchiveAssetUseCase::new(Arc::new(account_svc), Arc::new(asset_svc));
        let err = uc.archive_asset("asset-1").await.unwrap_err();
        assert!(
            matches!(
                err,
                ArchiveAssetError::Application(ArchiveAssetTask::ActiveHoldings)
            ),
            "got: {err:?}"
        );
    }

    // OQ-6 — archive succeeds when no active holdings exist
    #[tokio::test]
    async fn archive_succeeds_when_no_active_holdings() {
        let mut account_svc = MockAccountServiceContract::new();
        account_svc
            .expect_has_active_holdings_for_asset()
            .once()
            .with(eq("asset-1"))
            .returning(|_| Ok(false));
        let mut asset_svc = MockAssetServiceContract::new();
        asset_svc
            .expect_archive_asset()
            .once()
            .with(eq("asset-1"))
            .returning(|_| Ok(()));

        let uc = ArchiveAssetUseCase::new(Arc::new(account_svc), Arc::new(asset_svc));
        uc.archive_asset("asset-1").await.unwrap();
    }

    // Cross-BC guard failure propagates as the account arm of the composite.
    #[tokio::test]
    async fn archive_propagates_account_guard_failure() {
        use crate::context::account::AccountError;
        let mut account_svc = MockAccountServiceContract::new();
        account_svc
            .expect_has_active_holdings_for_asset()
            .once()
            .returning(|_| Err(AccountError::DatabaseError));
        let asset_svc = MockAssetServiceContract::new();

        let uc = ArchiveAssetUseCase::new(Arc::new(account_svc), Arc::new(asset_svc));
        let err = uc.archive_asset("asset-1").await.unwrap_err();
        assert!(
            matches!(err, ArchiveAssetError::Account(AccountError::DatabaseError)),
            "got: {err:?}"
        );
    }
}
