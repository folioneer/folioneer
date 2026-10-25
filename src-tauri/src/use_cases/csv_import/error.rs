use crate::context::account::AccountError;
use crate::context::import::ImportError;

/// Use-case rejections of an import: reading the file the user picked and the matches made
/// now (IMP-010, IMP-033).
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone, PartialEq, Eq)]
#[serde(tag = "code")]
pub enum CsvImportTask {
    /// The file cannot be read (IMP-010).
    #[error("The file cannot be read")]
    FileUnreadable,
    /// The file was not picked by the user in the system dialog (IMP-010).
    #[error("The file was not picked")]
    FileNotPicked,
    /// An invariant of the use case broke: a bug, logged where it happened.
    #[error("Unexpected error")]
    UnknownError,
    /// A match made to an asset the portfolio does not hold (IMP-033).
    #[error("Asset not found: {asset_id}")]
    MatchedAssetNotFound {
        /// The asset the match names.
        asset_id: String,
    },
}

/// Use-case composite for the import failure surface.
///
/// - `ImportError` — the file's content and the mapping (IMP-010 to IMP-021).
/// - `AccountError` — the account the file is read for (`AccountNotFound`, IMP-010).
/// - `CsvImportTask` — use-case-owned (this file).
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type)]
#[serde(untagged)]
pub enum CsvImportError {
    /// Import-BC rejection.
    #[error(transparent)]
    Import(#[from] ImportError),
    /// Account-BC rejection.
    #[error(transparent)]
    Account(#[from] AccountError),
    /// Use-case-layer rejection.
    #[error(transparent)]
    UseCase(#[from] CsvImportTask),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn csv_import_errors_serialize_as_their_code() {
        assert_eq!(
            to_value(CsvImportError::UseCase(
                CsvImportTask::MatchedAssetNotFound {
                    asset_id: "x-1".into()
                }
            ))
            .expect("serializes"),
            json!({ "code": "MatchedAssetNotFound", "asset_id": "x-1" })
        );
        assert_eq!(
            to_value(CsvImportError::UseCase(CsvImportTask::FileUnreadable)).expect("serializes"),
            json!({ "code": "FileUnreadable" })
        );
        assert_eq!(
            to_value(CsvImportError::Account(AccountError::AccountNotFound {
                account_id: "a-1".into()
            }))
            .expect("serializes"),
            json!({ "code": "AccountNotFound", "account_id": "a-1" })
        );
        assert_eq!(
            to_value(CsvImportError::Import(ImportError::FileEmpty)).expect("serializes"),
            json!({ "code": "FileEmpty" })
        );
        for task in [CsvImportTask::FileNotPicked, CsvImportTask::UnknownError] {
            let code = format!("{task:?}");
            assert_eq!(
                to_value(CsvImportError::UseCase(task)).expect("serializes"),
                json!({ "code": code })
            );
        }
    }

    // Both contexts' `DatabaseError` reach the window as one code, on purpose: the window
    // shows the same message whichever database failed.
    #[test]
    fn a_database_error_shares_one_code_across_both_contexts() {
        let import =
            to_value(CsvImportError::Import(ImportError::DatabaseError)).expect("serializes");
        let account =
            to_value(CsvImportError::Account(AccountError::DatabaseError)).expect("serializes");
        assert_eq!(import, json!({ "code": "DatabaseError" }));
        assert_eq!(import, account);
    }
}
