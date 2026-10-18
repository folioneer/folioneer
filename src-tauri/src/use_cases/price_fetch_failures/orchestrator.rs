//! Reads why this installation's last fetch could not price each asset (MKT-221).

use std::sync::Arc;

use super::error::PriceFetchFailuresError;
use crate::context::asset::AssetServiceContract;
use crate::core::logger::BACKEND;
use crate::use_cases::shared::price_fetch_failures::{
    PriceFetchFailure, PriceFetchFailureRepository,
};

/// Cross-context read over this installation's fetch failures and the assets.
pub struct PriceFetchFailuresUseCase {
    asset_service: Arc<dyn AssetServiceContract>,
    failures: Arc<dyn PriceFetchFailureRepository>,
}

impl PriceFetchFailuresUseCase {
    /// Creates the use case over its two read sources.
    pub fn new(
        asset_service: Arc<dyn AssetServiceContract>,
        failures: Arc<dyn PriceFetchFailureRepository>,
    ) -> Self {
        Self {
            asset_service,
            failures,
        }
    }

    /// MKT-221 — every recorded failure, but those of an asset whose price is typed by hand
    /// (MKT-150): no fetch attempts it any more, and the row says so instead.
    pub async fn read(&self) -> Result<Vec<PriceFetchFailure>, PriceFetchFailuresError> {
        let failures = self.failures.all().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "price_fetch_failures: reading the failures failed");
            PriceFetchFailuresError::DatabaseError
        })?;
        let mut shown = Vec::with_capacity(failures.len());
        for failure in failures {
            let asset = self
                .asset_service
                .get_asset_by_id(&failure.asset_id)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, asset_id = %failure.asset_id, err = ?e, "price_fetch_failures: reading an asset failed");
                    PriceFetchFailuresError::DatabaseError
                })?;
            if asset.is_some_and(|asset| !asset.price_refresh_blocked) {
                shown.push(failure);
            }
        }
        Ok(shown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::asset::{
        Asset, AssetCategory, AssetClass, AssetKind, MockAssetServiceContract, SYSTEM_CATEGORY_ID,
    };
    use crate::use_cases::shared::price_fetch_failures::{
        MockPriceFetchFailureRepository, PriceFetchFailureReason,
    };

    fn failure(asset_id: &str) -> PriceFetchFailure {
        PriceFetchFailure {
            asset_id: asset_id.to_string(),
            reason: PriceFetchFailureReason::NoData,
            failed_at: "2026-10-18T08:00:00".to_string(),
        }
    }

    fn asset(id: &str, typed_by_hand: bool) -> Asset {
        Asset::restore(
            id.to_string(),
            AssetKind::Crypto,
            id.to_string(),
            AssetClass::DigitalAsset,
            AssetCategory::from_storage(
                SYSTEM_CATEGORY_ID.to_string(),
                "generic.uncategorized".to_string(),
            ),
            "EUR".to_string(),
            5,
            id.to_string(),
            None,
            false,
            None,
            typed_by_hand,
            false,
        )
    }

    // MKT-221 — the failures are read as recorded, but an asset typed by hand says that
    // instead, and an asset no longer there says nothing.
    #[tokio::test]
    async fn mkt_221_reads_the_failures_but_those_of_a_price_typed_by_hand() {
        let mut failures = MockPriceFetchFailureRepository::new();
        failures
            .expect_all()
            .returning(|| Ok(vec![failure("fetched"), failure("typed"), failure("gone")]));
        let mut assets = MockAssetServiceContract::new();
        assets.expect_get_asset_by_id().returning(|id| {
            Ok(match id {
                "fetched" => Some(asset("fetched", false)),
                "typed" => Some(asset("typed", true)),
                _ => None,
            })
        });
        let uc = PriceFetchFailuresUseCase::new(Arc::new(assets), Arc::new(failures));
        assert_eq!(uc.read().await, Ok(vec![failure("fetched")]));
    }

    // MKT-221 — a failed read is a database error, never a partial list.
    #[tokio::test]
    async fn a_failed_read_is_a_database_error() {
        let mut failures = MockPriceFetchFailureRepository::new();
        failures
            .expect_all()
            .returning(|| Err(anyhow::anyhow!("disk")));
        let uc = PriceFetchFailuresUseCase::new(
            Arc::new(MockAssetServiceContract::new()),
            Arc::new(failures),
        );
        assert_eq!(uc.read().await, Err(PriceFetchFailuresError::DatabaseError));
    }
}
