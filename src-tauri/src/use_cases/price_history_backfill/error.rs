use crate::context::account::AccountError;
use crate::context::asset::AssetError;

/// Rejections owned by the price history backfill use case (MKT-195/196) — the
/// guards neither the account nor the asset context raises.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone, PartialEq)]
#[serde(tag = "code")]
pub enum PriceHistoryBackfillTask {
    /// The account has no transaction on the asset (MKT-191/196).
    #[error("The account never held this asset")]
    AssetNeverHeld,
    /// Automated price fetches are blocked for the asset (MKT-151).
    #[error("Automatic price updates are blocked for this asset")]
    PriceRefreshBlocked,
    /// No provider symbol can be derived, the provider reports the symbol unknown,
    /// or it serves no close over a held period of 7 days or more (MKT-196).
    #[error("No price history found for this asset's ticker")]
    TickerNotResolved,
    /// A daily-close request failed; nothing was recorded (MKT-195).
    #[error("The price provider could not be reached")]
    ProviderUnreachable,
}

/// Use-case composite for `backfill_holding_price_history`: the account and asset
/// context rejections propagated verbatim, plus the use case's own guards. The two
/// context enums share some codes (`DatabaseError`, `InvalidCurrency`, `DateInFuture`)
/// with identical payloads, so either leaf produces the same wire value.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum PriceHistoryBackfillError {
    /// Account context rejection (`AccountNotFound`, `DatabaseError`).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Asset context rejection (`AssetNotFound`, `CashAssetNotEditable`, `Archived`,
    /// `DatabaseError`).
    #[error(transparent)]
    Asset(#[from] AssetError),
    /// Use-case guard (`AssetNeverHeld`, `PriceRefreshBlocked`, `TickerNotResolved`,
    /// `ProviderUnreachable`).
    #[error(transparent)]
    Task(#[from] PriceHistoryBackfillTask),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    // error-model.md wire-shape check — every leaf flattens to { "code": ... }.
    #[test]
    fn each_leaf_emits_a_code() {
        let account_not_found: PriceHistoryBackfillError = AccountError::AccountNotFound {
            account_id: "acc-1".into(),
        }
        .into();
        assert_eq!(
            to_value(&account_not_found).unwrap(),
            json!({ "code": "AccountNotFound", "account_id": "acc-1" })
        );
        let archived: PriceHistoryBackfillError = AssetError::Archived.into();
        assert_eq!(to_value(&archived).unwrap(), json!({ "code": "Archived" }));
        for (task, code) in [
            (PriceHistoryBackfillTask::AssetNeverHeld, "AssetNeverHeld"),
            (
                PriceHistoryBackfillTask::PriceRefreshBlocked,
                "PriceRefreshBlocked",
            ),
            (
                PriceHistoryBackfillTask::TickerNotResolved,
                "TickerNotResolved",
            ),
            (
                PriceHistoryBackfillTask::ProviderUnreachable,
                "ProviderUnreachable",
            ),
        ] {
            let composite: PriceHistoryBackfillError = task.into();
            assert_eq!(to_value(&composite).unwrap(), json!({ "code": code }));
        }
    }
}
