use crate::context::asset::AssetServiceContract;
use anyhow::Result;
use std::sync::Arc;

/// Ensures the system Cash Asset for `currency` exists in the asset catalog (CSH-010).
///
/// Delegates the idempotent upsert (cash category + Cash Asset, CSH-011 / CSH-017) to
/// `AssetService::seed_cash_asset`. Safe to call from every cash-affecting use case;
/// returns `Ok(())` whether the asset was newly created or already present.
pub async fn ensure_cash_asset(
    asset_service: &Arc<dyn AssetServiceContract>,
    currency: &str,
) -> Result<()> {
    asset_service.seed_cash_asset(currency).await.map(|_| ())
}
