use crate::context::account::AccountError;
use crate::context::asset::AssetError;

/// Application-layer rejection specific to the `archive_asset` use case —
/// the cross-BC active-holdings check performed by the orchestrator before
/// delegating to `AssetService::archive_asset`.
///
/// Per the rejection-layer rule (`docs/ddd-reference.md` § Errors): this
/// rejection is born at the orchestrator (it queries the account service and
/// decides whether to proceed), not by an aggregate method on its own loaded
/// state — application-class.
///
/// Tagged with `#[serde(tag = "code")]` so it serializes verbatim across the
/// Tauri boundary into a flat `{ code: "..." }` shape through the
/// `ArchiveAssetError` untagged composite.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum ArchiveAssetTask {
    /// Asset still has non-zero holdings in at least one account (OQ-6).
    #[error("Cannot archive an asset with active holdings")]
    ActiveHoldings,
}

/// Use-case composite for the **archive asset** failure surface — the single
/// command `archive_asset` (OQ-6) and its full chain of rejections.
///
/// This IS the FE-facing contract for the `archive_asset` Tauri command — each
/// leaf already serializes with `#[serde(tag = "code")]`, and `#[serde(untagged)]`
/// here flattens them into a single FE-visible union.
///
/// Each leaf lives in its rightful layer:
/// - `AssetError` — asset BC enum, carries `AssetNotFound` and
///   `CashAssetNotEditable` propagated verbatim per the
///   composition-over-redefinition rule.
/// - `AccountError` — account BC, surfaces `DatabaseError` from the cross-BC
///   active-holdings check.
/// - `ArchiveAssetTask` — use-case-owned (this file), raises
///   `ActiveHoldings` from the orchestrator.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum ArchiveAssetError {
    /// Asset BC rejection (`AssetNotFound`, `CashAssetNotEditable`, propagated
    /// `DatabaseError`).
    #[error(transparent)]
    Asset(#[from] AssetError),
    /// Account BC rejection (`DatabaseError` from the cross-BC
    /// active-holdings check).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case orchestration rejection (`ActiveHoldings`).
    #[error(transparent)]
    Application(#[from] ArchiveAssetTask),
}

#[cfg(test)]
mod tests {
    use super::*;

    // CSH-016 — domain CashAssetNotEditable propagates through Asset leaf
    #[test]
    fn cash_asset_not_editable_propagates_through_asset_leaf() {
        let composite: ArchiveAssetError = AssetError::CashAssetNotEditable.into();
        assert!(
            matches!(
                composite,
                ArchiveAssetError::Asset(AssetError::CashAssetNotEditable)
            ),
            "got: {composite:?}"
        );
    }

    // Asset-side AssetNotFound propagates verbatim with id payload preserved
    #[test]
    fn asset_not_found_propagates_with_id_payload() {
        let composite: ArchiveAssetError = AssetError::AssetNotFound {
            id: "missing-asset".into(),
        }
        .into();
        assert!(
            matches!(
                &composite,
                ArchiveAssetError::Asset(AssetError::AssetNotFound { id }) if id == "missing-asset"
            ),
            "got: {composite:?}"
        );
    }

    // Account-side DatabaseError surfaces through the Account leaf
    // (cross-BC active-holdings check repo failure)
    #[test]
    fn account_database_error_surfaces_through_account_leaf() {
        let composite: ArchiveAssetError = AccountError::DatabaseError.into();
        assert!(
            matches!(
                composite,
                ArchiveAssetError::Account(AccountError::DatabaseError)
            ),
            "got: {composite:?}"
        );
    }

    // OQ-6 — ActiveHoldings surfaces through the Application leaf
    #[test]
    fn active_holdings_surfaces_through_application_leaf() {
        let composite: ArchiveAssetError = ArchiveAssetTask::ActiveHoldings.into();
        assert!(
            matches!(
                composite,
                ArchiveAssetError::Application(ArchiveAssetTask::ActiveHoldings)
            ),
            "got: {composite:?}"
        );
    }
}
