#[cfg(test)]
mod tests {
    use super::super::{FetchAllAssetPricesError, FetchPriceTask};
    use crate::context::account::AccountError;
    use crate::context::asset::AssetError;

    fn json(error: &FetchAllAssetPricesError) -> serde_json::Value {
        serde_json::to_value(error).expect("serialize")
    }

    #[test]
    fn use_case_variants_serialize_with_code() {
        let already = FetchAllAssetPricesError::Failure(FetchPriceTask::FetchAlreadyRunning);
        let empty = FetchAllAssetPricesError::Failure(FetchPriceTask::NoFetchableHoldings);
        let unknown = FetchAllAssetPricesError::Failure(FetchPriceTask::UnknownError);
        assert_eq!(
            json(&already),
            serde_json::json!({ "code": "FetchAlreadyRunning" })
        );
        assert_eq!(
            json(&empty),
            serde_json::json!({ "code": "NoFetchableHoldings" })
        );
        assert_eq!(
            json(&unknown),
            serde_json::json!({ "code": "UnknownError" })
        );
    }

    #[test]
    fn asset_wrapper_flattens_bc_code() {
        let wrapped = FetchAllAssetPricesError::Asset(AssetError::DatabaseError);
        assert_eq!(
            json(&wrapped),
            serde_json::json!({ "code": "DatabaseError" })
        );
    }

    #[test]
    fn account_wrapper_flattens_bc_code_with_payload() {
        let wrapped = FetchAllAssetPricesError::Account(AccountError::AccountNotFound {
            account_id: "abc".into(),
        });
        assert_eq!(
            json(&wrapped),
            serde_json::json!({ "code": "AccountNotFound", "account_id": "abc" })
        );
    }

    // PMV-010/011/014 — the `AssetPriceFetchCompleted.movement` field serializes
    // as the contract declares: absent (null) when no report was produced, a
    // present object for a captured Manual-trigger report. Field-shape coverage
    // for `PriceMovementReport` itself lives in `core::event_bus::event`.
    #[test]
    fn asset_price_fetch_completed_movement_matches_the_contract_shape() {
        use crate::core::event_bus::{Event, PriceMovementReport};

        let absent = serde_json::to_value(Event::AssetPriceFetchCompleted {
            ok: 0,
            skipped: 0,
            unpriced: vec![],
            movement: None,
        })
        .expect("serialize");
        assert_eq!(absent.get("movement"), Some(&serde_json::Value::Null));

        let present = serde_json::to_value(Event::AssetPriceFetchCompleted {
            ok: 1,
            skipped: 0,
            unpriced: vec![],
            movement: Some(PriceMovementReport {
                rows: vec![],
                total_before: 0,
                total_after: 0,
                total_currency: "EUR".to_string(),
                total_movement_pct: None,
                total_movement_amount: None,
                observed_from: None,
                observed_to: None,
                incomplete: false,
            }),
        })
        .expect("serialize");
        assert!(
            present.get("movement").is_some_and(|m| m.is_object()),
            "expected a present movement object, got: {present:?}"
        );
    }
}
