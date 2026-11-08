//! Wire-facing error surface for the data export (EXP-015, docs/error-model.md).

use serde::Serialize;
use specta::Type;

/// Flat error enum for `export_portfolio`; serializes as `{ "code": "..." }`.
#[derive(Debug, thiserror::Error, Serialize, Type, Clone, PartialEq)]
#[serde(tag = "code")]
pub enum DataExportError {
    /// EXP-015 — the chosen folder does not exist or is not a folder.
    #[error("The chosen folder is not available")]
    FolderUnavailable,
    /// EXP-015 — writing failed (a folder that cannot be written, a full disk); nothing of
    /// the export is left behind.
    #[error("The export could not be written")]
    WriteFailed,
    /// A read failed; the cause is logged server-side.
    #[error("An unexpected database error occurred")]
    DatabaseError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    // error-model.md wire-shape check — each variant emits a flat { "code": "..." }.
    #[test]
    fn each_variant_emits_a_code() {
        for (error, code) in [
            (DataExportError::FolderUnavailable, "FolderUnavailable"),
            (DataExportError::WriteFailed, "WriteFailed"),
            (DataExportError::DatabaseError, "DatabaseError"),
        ] {
            assert_eq!(to_value(error).unwrap(), json!({ "code": code }));
        }
    }
}
