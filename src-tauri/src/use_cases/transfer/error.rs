use crate::context::account::AccountError;

/// Application-layer rejections of a transfer — the cross-BC asset checks performed by the
/// orchestrator before delegating to the account context (TRF-010).
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone, PartialEq, Eq)]
#[serde(tag = "code")]
pub enum TransferTask {
    /// No asset exists with the requested ID (TRF-010).
    #[error("Asset not found")]
    AssetNotFound,
    /// The asset is archived — nothing of it is transferred (TRF-010, as SEL-037).
    #[error("Cannot transfer an archived asset")]
    ArchivedAsset,
    /// The asset is a Cash Asset — cash moves by a withdrawal and a deposit (TRF-010).
    #[error("Cash cannot be transferred")]
    TransferOfCash,
}

/// Use-case composite for the transfer failure surface.
///
/// - `AccountError` — every account-BC rejection (lookup, infrastructure, the figures
///   TRF-011, the replay guards TRF-012/030/040, the rate TRF-022).
/// - `TransferTask` — use-case-owned (this file), the cross-BC asset checks.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum TransferError {
    /// Account-BC rejection.
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-layer rejection (cross-BC asset checks).
    #[error(transparent)]
    UseCase(#[from] TransferTask),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn transfer_errors_serialize_as_their_code() {
        assert_eq!(
            to_value(TransferError::UseCase(TransferTask::TransferOfCash)).unwrap(),
            json!({ "code": "TransferOfCash" })
        );
        assert_eq!(
            to_value(TransferError::Account(AccountError::TransferNotFound)).unwrap(),
            json!({ "code": "TransferNotFound" })
        );
    }
}
