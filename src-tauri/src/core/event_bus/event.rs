//! Domain events published across bounded contexts.

use serde::Serialize;

/// All possible side-effect events that can be published across the application.
/// Each variant represents a specific business event that features may need to react to.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, specta::Type, tauri_specta::Event)]
#[serde(tag = "type")]
pub enum Event {
    /// Health check event for testing/monitoring
    Health,
    /// An asset was created, updated, or deleted
    AssetUpdated,
    /// An account was created, updated, or deleted
    AccountUpdated,
    /// A category was created, updated, or deleted
    CategoryUpdated,
    /// A transaction was created, updated, or deleted (position data changed)
    TransactionUpdated,
    /// A market price was recorded or updated for an asset (MKT-026)
    AssetPriceUpdated,
    /// Progress of an in-flight price-fetch task (MKT-180): emitted once at task
    /// start (`done = 0`) and after each attempted asset, successful or skipped.
    /// The frontend drives the shell progress bar from it.
    AssetPriceFetchProgress {
        /// Assets attempted so far (successful + skipped).
        done: u32,
        /// Total assets in the fetch scope.
        total: u32,
    },
    /// A price-fetch task finished: `ok` assets were updated, `skipped` were not
    /// (no data or fetch failure). Carries counts so the frontend can summarize
    /// the outcome (MKT-119), plus the per-asset unpriced list so it can offer
    /// manual entry (MKT-170). Distinct from the per-asset `AssetPriceUpdated`.
    AssetPriceFetchCompleted {
        /// Count of assets whose price was successfully updated.
        ok: u32,
        /// Count of assets skipped — no data, or a fetch/upsert failure.
        skipped: u32,
        /// The skipped assets, one entry each (MKT-170/171); `len() == skipped`.
        unpriced: Vec<UnpricedAsset>,
        /// What this refresh did to each account's value (PMV-010/011): present
        /// only for a Global refresh (`trigger == Manual`); absent on the launch
        /// auto-fetch, on every account-scoped fetch, and when the report could
        /// not be produced (PMV-014).
        movement: Option<PriceMovementReport>,
    },
    /// A currency rate was recorded, updated, or deleted (FXR-026/052/053/074).
    CurrencyRateUpdated,
    /// A currency pair was declared or removed — by this device (FXR-054) or applied from
    /// another one (SYN-064).
    CurrencyPairUpdated,
    /// A holding note was written or deleted — by this device (HNO-020/021) or applied from
    /// another one (SYN-064).
    HoldingNoteUpdated,
    /// A fee schedule was created, updated, paused, reactivated, or deleted (FEE-064).
    FeeScheduleUpdated,
    /// A sync run completed — automatic, launch, `sync_now`, or `resume_sync` — that applied
    /// at least one change or whose failures/paused state changed since the previous run
    /// (SYN-063/064). A bare marker: the frontend treats it as a global refresh and re-reads
    /// `get_sync_status`.
    SyncCompleted,
}

/// One asset a price-fetch task could not price (MKT-170/171), carried in the
/// `AssetPriceFetchCompleted` payload so the frontend can list it for manual entry.
/// `last_price` / `last_price_date` describe the asset's most recently recorded
/// price and are absent when the asset has never had a price recorded.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, specta::Type)]
pub struct UnpricedAsset {
    /// The asset whose price could not be updated.
    pub asset_id: String,
    /// The asset's display name.
    pub name: String,
    /// The asset's ticker / free-form reference.
    pub reference: String,
    /// The asset's ISIN, when it has one.
    pub isin: Option<String>,
    /// ISO 4217 currency code the asset's prices are denominated in.
    pub currency: String,
    /// Most recently recorded price in the asset's native currency, i64 micros
    /// (ADR-001); absent when the asset has never had a price recorded.
    pub last_price: Option<i64>,
    /// ISO 8601 date of `last_price`; absent when there is no recorded price.
    pub last_price_date: Option<String>,
}

/// PMV-020+ — what a manual Global refresh did to the portfolio's value. Both
/// readings are computed over the same holdings, quantities and rates, so
/// only prices differ between them (PMV-020). Both readings use the rates in
/// force when the refresh STARTED. Produced only when `trigger == Manual`;
/// carried in `AssetPriceFetchCompleted`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, specta::Type)]
pub struct PriceMovementReport {
    /// One entry per account, by account name ascending (PMV-030/033).
    pub rows: Vec<PriceMovementRow>,
    /// Portfolio value before the fetch, reference-currency micros (PMV-040, GPF-011, ADR-001).
    pub total_before: i64,
    /// Portfolio value after the fetch, reference-currency micros (PMV-040).
    pub total_after: i64,
    /// The reference currency both totals are expressed in (PMV-040).
    pub total_currency: String,
    /// Micro-percent movement derived from the two totals (PMV-041); absent
    /// when the earlier total is not positive (PMV-044) or the two totals are
    /// equal (PMV-045).
    pub total_movement_pct: Option<i64>,
    /// Signed movement in reference-currency micros, `total_after - total_before`
    /// (PMV-046); absent when the two totals are equal (PMV-045), present even when
    /// `total_before` is not positive (PMV-044).
    pub total_movement_amount: Option<i64>,
    /// ISO date carried before the fetch (PMV-050); absent per PMV-052.
    pub observed_from: Option<String>,
    /// ISO date this fetch produced (PMV-050); absent when the fetch produced
    /// none later than `observed_from` (PMV-051); still carried even when
    /// `observed_from` is absent (PMV-052).
    pub observed_to: Option<String>,
    /// Whether any row's reading is incomplete (PMV-043).
    pub incomplete: bool,
}

/// PMV-030 — one account's share of the report. Present for every account,
/// including those that did not move and those holding no priced asset.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, specta::Type)]
pub struct PriceMovementRow {
    /// The account this entry describes.
    pub account_id: String,
    /// The account's display name.
    pub name: String,
    /// The account's own currency — both values are in it (PMV-034).
    pub currency: String,
    /// Account value before the fetch, account-currency micros (PMV-021).
    pub before: i64,
    /// Account value after the fetch, account-currency micros (PMV-022).
    pub after: i64,
    /// Micro-percent movement (PMV-024); absent when unmoved (PMV-031) or when
    /// `before` is not positive (PMV-025).
    pub movement_pct: Option<i64>,
    /// Signed movement in account-currency micros, `after - before` (PMV-027);
    /// absent when unmoved, present even when `before` is not positive.
    pub movement_amount: Option<i64>,
    /// A holding meant to be read at its current price could not be
    /// (PMV-032): the MKT-171 skip set, or one contributing 0 for want of a
    /// usable rate (FXR-034/GPF). System cash (MKT-116) and refresh-locked
    /// holdings (MKT-151) never set it.
    pub incomplete: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // SYN-064/D10 — SyncCompleted serializes with the `#[serde(tag = "type")]` discriminator
    // this enum already uses for every other variant, and carries no payload.
    #[test]
    fn sync_completed_serializes_with_its_type_tag() {
        let value = serde_json::to_value(Event::SyncCompleted).unwrap();
        assert_eq!(
            value.get("type").and_then(|t| t.as_str()),
            Some("SyncCompleted")
        );
        assert_eq!(value.as_object().map(|fields| fields.len()), Some(1));
    }

    fn sample_report() -> PriceMovementReport {
        PriceMovementReport {
            rows: vec![PriceMovementRow {
                account_id: "acc-1".to_string(),
                name: "Alpha".to_string(),
                currency: "EUR".to_string(),
                before: 100_000_000,
                after: 100_000_000,
                movement_pct: None,
                movement_amount: None,
                incomplete: false,
            }],
            total_before: 100_000_000,
            total_after: 100_000_000,
            total_currency: "EUR".to_string(),
            total_movement_pct: None,
            total_movement_amount: None,
            observed_from: None,
            observed_to: None,
            incomplete: false,
        }
    }

    // PMV-060 — the report states no count and no "moved" flag of its own; the
    // wire shape carries exactly the contract's nine fields, nothing more.
    // The frontend derives "nothing moved" from `rows` itself.
    #[test]
    fn price_movement_report_serializes_with_exactly_the_contract_fields() {
        let value = serde_json::to_value(sample_report()).unwrap();
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "incomplete",
                "observed_from",
                "observed_to",
                "rows",
                "total_after",
                "total_before",
                "total_currency",
                "total_movement_amount",
                "total_movement_pct",
            ]
        );
    }

    // PMV-010/011 — AssetPriceFetchCompleted carries `movement` as an absent
    // (null) field when no report was produced (launch auto-fetch, account
    // fetch, or a failed capture).
    #[test]
    fn asset_price_fetch_completed_serializes_absent_movement_as_null() {
        let value = serde_json::to_value(Event::AssetPriceFetchCompleted {
            ok: 1,
            skipped: 0,
            unpriced: vec![],
            movement: None,
        })
        .unwrap();
        assert_eq!(value.get("movement"), Some(&serde_json::Value::Null));
    }

    // PMV-010/011 — a Manual, successfully-captured report serializes as a
    // present object under `movement`.
    #[test]
    fn asset_price_fetch_completed_serializes_present_movement_as_an_object() {
        let value = serde_json::to_value(Event::AssetPriceFetchCompleted {
            ok: 1,
            skipped: 0,
            unpriced: vec![],
            movement: Some(sample_report()),
        })
        .unwrap();
        assert!(
            value.get("movement").is_some_and(|m| m.is_object()),
            "expected movement to serialize as a present object, got: {value:?}"
        );
    }
}
