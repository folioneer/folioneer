//! Shared fetch-scope builder (SPF-040) reused by `asset_price_fetch` and
//! `scheduled_fetch` so the MKT-116 (system cash) / MKT-151 (refresh-locked)
//! exclusions are defined once. Extracted from
//! `use_cases::asset_price_fetch::orchestrator::build_scope`.

use crate::context::asset::{
    derive_yahoo_symbol_with_exchange, Asset, AssetError, AssetServiceContract,
};
use crate::context::currency::CurrencyPair;
use crate::core::cash::is_cash_asset;
use crate::core::logger::BACKEND;
use std::collections::{HashMap, HashSet};

/// Loads every asset in `asset_ids` once and returns the fetchable `scope`
/// (assets with a derivable Yahoo symbol and an unlocked price refresh, MKT-110,
/// MKT-151, ADR-014) alongside an `asset_id → currency` map covering all loaded
/// assets — including locked and non-derivable ones, which are excluded from
/// scope but still needed by FX-pair derivation (FXR-071). System cash assets
/// (MKT-116) never enter the map at all.
pub async fn build_scope(
    asset_service: &dyn AssetServiceContract,
    asset_ids: HashSet<String>,
) -> Result<(Vec<(Asset, String)>, HashMap<String, String>), AssetError> {
    let mut scope: Vec<(Asset, String)> = Vec::new();
    let mut currency_by_asset: HashMap<String, String> = HashMap::new();
    for asset_id in asset_ids {
        if is_cash_asset(&asset_id) {
            continue;
        }
        let asset = match asset_service.get_asset_by_id(&asset_id).await {
            Ok(Some(asset)) => asset,
            Ok(None) => continue,
            Err(application_error) => {
                tracing::error!(
                    target: BACKEND,
                    asset_id = %asset_id,
                    err = ?application_error,
                    "fetch_scope: get_asset_by_id failed"
                );
                return Err(application_error);
            }
        };
        currency_by_asset.insert(asset_id, asset.currency.clone());
        // MKT-151 / ADR-014 — a locked asset is excluded from fetch scope,
        // preserving its most recently recorded price (same shape as the
        // system-cash exclusion above).
        if asset.price_refresh_blocked {
            continue;
        }
        let Some(symbol) =
            derive_yahoo_symbol_with_exchange(&asset.reference, asset.exchange.as_ref())
        else {
            continue;
        };
        scope.push((asset, symbol));
    }
    Ok((scope, currency_by_asset))
}

/// Derives the distinct foreign-currency `CurrencyPair`s (`asset_currency →
/// account_currency`) for the active holdings in `inputs` (FXR-071/013), reading
/// each asset's currency from the `currency_by_asset` map produced by
/// `build_scope`. Cash holdings and same-currency holdings are excluded; an asset
/// absent from the map (cash, not found) is skipped.
pub fn build_fx_pairs(
    inputs: Vec<(String, String)>,
    currency_by_asset: &HashMap<String, String>,
) -> Vec<CurrencyPair> {
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut pairs: Vec<CurrencyPair> = Vec::new();

    for (account_currency, asset_id) in inputs {
        if is_cash_asset(&asset_id) {
            continue;
        }
        let Some(asset_currency) = currency_by_asset.get(&asset_id) else {
            continue;
        };
        if *asset_currency == account_currency {
            continue;
        }
        if seen.insert((asset_currency.clone(), account_currency.clone())) {
            pairs.push(CurrencyPair::from_storage(
                asset_currency.clone(),
                account_currency,
            ));
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::asset::{
        AssetCategory, AssetClass, MockAssetServiceContract, SYSTEM_CATEGORY_ID,
    };
    use crate::core::cash::system_cash_asset_id;

    fn make_category() -> AssetCategory {
        AssetCategory::from_storage(
            SYSTEM_CATEGORY_ID.to_string(),
            "generic.uncategorized".to_string(),
        )
    }

    fn make_asset(id: &str, reference: &str, locked: bool) -> Asset {
        Asset::restore(
            id.to_string(),
            "Test Asset".to_string(),
            AssetClass::Stocks,
            make_category(),
            "USD".to_string(),
            1,
            reference.to_string(),
            None,
            false,
            None,
            locked,
            false,
        )
    }

    // SPF-040 / MKT-116 — a system cash asset id is excluded before any asset lookup.
    #[tokio::test]
    async fn build_scope_excludes_system_cash_asset_without_lookup() {
        let cash_id = system_cash_asset_id("USD");
        let mut asset_service = MockAssetServiceContract::new();
        asset_service.expect_get_asset_by_id().times(0);

        let mut ids = HashSet::new();
        ids.insert(cash_id);
        let (scope, _) = build_scope(&asset_service, ids).await.unwrap();
        assert!(scope.is_empty(), "cash asset must never enter scope");
    }

    // SPF-040 / MKT-151 / ADR-014 — a refresh-locked asset is excluded from scope
    // but still appears in the currency map for FX-pair derivation.
    #[tokio::test]
    async fn build_scope_excludes_locked_asset_from_scope_but_keeps_currency_map() {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .times(1)
            .returning(|_| Ok(Some(make_asset("locked-id", "AAPL", true))));

        let mut ids = HashSet::new();
        ids.insert("locked-id".to_string());
        let (scope, currency_by_asset) = build_scope(&asset_service, ids).await.unwrap();
        assert!(
            scope.is_empty(),
            "a locked asset must not enter fetch scope"
        );
        assert_eq!(
            currency_by_asset.get("locked-id").map(String::as_str),
            Some("USD"),
            "a locked asset must still appear in the currency map (FXR-071)"
        );
    }

    // SPF-040 / MKT-110 — an asset with no derivable Yahoo symbol is excluded from scope.
    #[tokio::test]
    async fn build_scope_excludes_asset_with_no_derivable_symbol() {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .times(1)
            .returning(|_| Ok(Some(make_asset("no-ref-id", "", false))));

        let mut ids = HashSet::new();
        ids.insert("no-ref-id".to_string());
        let (scope, _) = build_scope(&asset_service, ids).await.unwrap();
        assert!(
            scope.is_empty(),
            "an asset with no derivable provider symbol must not enter scope"
        );
    }

    // SPF-040 — an eligible asset (unlocked, derivable symbol) enters the scope.
    #[tokio::test]
    async fn build_scope_includes_eligible_asset() {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .times(1)
            .returning(|_| Ok(Some(make_asset("aapl-id", "AAPL", false))));

        let mut ids = HashSet::new();
        ids.insert("aapl-id".to_string());
        let (scope, currency_by_asset) = build_scope(&asset_service, ids).await.unwrap();
        assert_eq!(scope.len(), 1, "an eligible asset must enter scope");
        assert_eq!(scope[0].0.id, "aapl-id");
        assert_eq!(
            currency_by_asset.get("aapl-id").map(String::as_str),
            Some("USD")
        );
    }

    // SPF-040 — a repository failure surfaces as the typed AssetError::DatabaseError.
    #[tokio::test]
    async fn build_scope_surfaces_database_error_on_lookup_failure() {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .times(1)
            .returning(|_| Err(AssetError::DatabaseError));

        let mut ids = HashSet::new();
        ids.insert("broken-id".to_string());
        let error = build_scope(&asset_service, ids).await.unwrap_err();
        assert!(matches!(error, AssetError::DatabaseError), "got: {error:?}");
    }
}
