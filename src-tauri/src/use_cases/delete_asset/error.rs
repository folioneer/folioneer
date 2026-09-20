use crate::context::account::AccountError;
use crate::context::asset::AssetError;

/// Application-layer rejection specific to the `delete_asset` use case —
/// the cross-BC transaction-history check performed by the orchestrator
/// before delegating to `AssetService::delete_asset`.
///
/// Per the rejection-layer rule (`docs/ddd-reference.md` § Errors): this
/// rejection is born at the orchestrator (it queries the account service and
/// decides whether to proceed), not by an aggregate method on its own loaded
/// state — application-class.
///
/// Tagged with `#[serde(tag = "code")]` so it serializes verbatim across the
/// Tauri boundary into a flat `{ code: "..." }` shape through the
/// `DeleteAssetError` untagged composite.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum DeleteAssetTask {
    /// At least one transaction references this asset; deletion would break history.
    #[error("Cannot delete an asset with existing transactions")]
    ExistingTransactions,
}

/// Use-case composite for the **delete asset** failure surface — the single
/// command `delete_asset` and its full chain of rejections.
///
/// This IS the FE-facing contract for the `delete_asset` Tauri command — each
/// leaf already serializes with `#[serde(tag = "code")]`, and `#[serde(untagged)]`
/// here flattens them into a single FE-visible union.
///
/// Each leaf lives in its rightful layer:
/// - `AssetError` — asset BC enum, carries `AssetNotFound` and
///   `CashAssetNotEditable` propagated verbatim per the
///   composition-over-redefinition rule.
/// - `AccountError` — account BC, surfaces `DatabaseError` from the cross-BC
///   transaction-history check.
/// - `DeleteAssetTask` — use-case-owned (this file), raises
///   `ExistingTransactions` from the orchestrator.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum DeleteAssetError {
    /// Asset BC rejection (`AssetNotFound`, `CashAssetNotEditable`, propagated
    /// `DatabaseError`).
    #[error(transparent)]
    Asset(#[from] AssetError),
    /// Account BC rejection (`DatabaseError` from the cross-BC
    /// transaction-history check).
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case orchestration rejection (`ExistingTransactions`).
    #[error(transparent)]
    Application(#[from] DeleteAssetTask),
}

#[cfg(test)]
mod tests {
    use super::*;

    // CSH-016 — domain CashAssetNotEditable propagates through Asset leaf
    #[test]
    fn cash_asset_not_editable_propagates_through_asset_leaf() {
        let composite: DeleteAssetError = AssetError::CashAssetNotEditable.into();
        assert!(
            matches!(
                composite,
                DeleteAssetError::Asset(AssetError::CashAssetNotEditable)
            ),
            "got: {composite:?}"
        );
    }

    // Asset-side AssetNotFound propagates verbatim with id payload preserved
    #[test]
    fn asset_not_found_propagates_with_id_payload() {
        let composite: DeleteAssetError = AssetError::AssetNotFound {
            id: "missing-asset".into(),
        }
        .into();
        assert!(
            matches!(
                &composite,
                DeleteAssetError::Asset(AssetError::AssetNotFound { id }) if id == "missing-asset"
            ),
            "got: {composite:?}"
        );
    }

    // Account-side DatabaseError surfaces through the Account leaf
    // (cross-BC transaction-history check repo failure)
    #[test]
    fn account_database_error_surfaces_through_account_leaf() {
        let composite: DeleteAssetError = AccountError::DatabaseError.into();
        assert!(
            matches!(
                composite,
                DeleteAssetError::Account(AccountError::DatabaseError)
            ),
            "got: {composite:?}"
        );
    }

    // ExistingTransactions surfaces through the Application leaf
    #[test]
    fn existing_transactions_surfaces_through_application_leaf() {
        let composite: DeleteAssetError = DeleteAssetTask::ExistingTransactions.into();
        assert!(
            matches!(
                composite,
                DeleteAssetError::Application(DeleteAssetTask::ExistingTransactions)
            ),
            "got: {composite:?}"
        );
    }
}
