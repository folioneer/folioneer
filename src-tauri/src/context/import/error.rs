/// Single flat error enum for the `import` bounded context (gold error model).
///
/// `#[serde(tag = "code")]` makes each variant serialize as `{ "code": "VariantName", ... }`
/// on the wire.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum ImportError {
    /// The mapping's name is empty once trimmed (IMP-024).
    #[error("A column mapping needs a name")]
    MappingNameEmpty,

    /// Another mapping holds the name, case and accents ignored (IMP-024).
    #[error("Another column mapping is named {name}")]
    MappingNameTaken {
        /// The name asked for.
        name: String,
    },

    /// The mapping names no date column, no operation column, or no asset column (IMP-021).
    #[error("A column mapping needs the date, the operation and an asset column")]
    MappingColumnMissing,

    /// A text of the mapping is longer than `MAPPING_TEXT_LENGTH` characters (IMP-024A).
    #[error("A text of the column mapping is too long")]
    MappingTextTooLong,

    /// The mapping is no longer held (IMP-025, IMP-040).
    #[error("Column mapping not found: {mapping_id}")]
    MappingNotFound {
        /// The mapping asked for.
        mapping_id: String,
    },

    /// The account the file is read for is unknown (IMP-010).
    #[error("Account not found: {account_id}")]
    AccountNotFound {
        /// The account asked for.
        account_id: String,
    },

    /// The file cannot be read (IMP-010).
    #[error("The file cannot be read")]
    FileUnreadable,

    /// A match made to an asset the portfolio does not hold (IMP-033).
    #[error("Asset not found: {asset_id}")]
    MatchedAssetNotFound {
        /// The asset the match names.
        asset_id: String,
    },

    /// The file is larger than 10 MB (IMP-010).
    #[error("The file is larger than 10 MB")]
    FileTooLarge,

    /// The file holds nothing, or its first line is blank (IMP-010).
    #[error("The file is empty")]
    FileEmpty,

    /// The file holds a header and no row that is not blank (IMP-010, IMP-016).
    #[error("The file holds no row after its header")]
    FileHasNoRow,

    /// The file was to be read as UTF-8 and its bytes are not (IMP-011).
    #[error("The file is not written in UTF-8")]
    FileNotUtf8,

    /// No separator splits the header and the lines alike (IMP-012).
    #[error("No separator was found in the file")]
    SeparatorNotFound,

    /// The header holds a name twice, case ignored (IMP-015).
    #[error("The header names the column {name} twice")]
    HeaderNameRepeated {
        /// The name held twice, as written the second time.
        name: String,
    },

    /// The mapping in use names a column the file lacks (IMP-021).
    #[error("The file has no column {column}")]
    MappedColumnNotInFile {
        /// The column's header, as the mapping names it.
        column: String,
    },

    /// An infrastructure failure; the diagnostic is logged server-side, never sent.
    #[error("An unexpected database error occurred")]
    DatabaseError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    // Every variant emits a flat `{ "code": … }` object on the wire.
    #[test]
    fn each_variant_emits_a_code() {
        assert_eq!(
            to_value(ImportError::MappingNameEmpty).expect("serializes"),
            json!({ "code": "MappingNameEmpty" })
        );
        assert_eq!(
            to_value(ImportError::MappingNameTaken { name: "PEA".into() }).expect("serializes"),
            json!({ "code": "MappingNameTaken", "name": "PEA" })
        );
        assert_eq!(
            to_value(ImportError::MappingColumnMissing).expect("serializes"),
            json!({ "code": "MappingColumnMissing" })
        );
        assert_eq!(
            to_value(ImportError::MappingTextTooLong).expect("serializes"),
            json!({ "code": "MappingTextTooLong" })
        );
        assert_eq!(
            to_value(ImportError::MappingNotFound {
                mapping_id: "m-1".into()
            })
            .expect("serializes"),
            json!({ "code": "MappingNotFound", "mapping_id": "m-1" })
        );
        assert_eq!(
            to_value(ImportError::HeaderNameRepeated {
                name: "Date".into()
            })
            .expect("serializes"),
            json!({ "code": "HeaderNameRepeated", "name": "Date" })
        );
        assert_eq!(
            to_value(ImportError::MappedColumnNotInFile {
                column: "Qté".into()
            })
            .expect("serializes"),
            json!({ "code": "MappedColumnNotInFile", "column": "Qté" })
        );
        assert_eq!(
            to_value(ImportError::AccountNotFound {
                account_id: "a-1".into()
            })
            .expect("serializes"),
            json!({ "code": "AccountNotFound", "account_id": "a-1" })
        );
        assert_eq!(
            to_value(ImportError::MatchedAssetNotFound {
                asset_id: "x-1".into()
            })
            .expect("serializes"),
            json!({ "code": "MatchedAssetNotFound", "asset_id": "x-1" })
        );
        for (error, code) in [
            (ImportError::FileUnreadable, "FileUnreadable"),
            (ImportError::FileTooLarge, "FileTooLarge"),
            (ImportError::FileEmpty, "FileEmpty"),
            (ImportError::FileHasNoRow, "FileHasNoRow"),
            (ImportError::FileNotUtf8, "FileNotUtf8"),
            (ImportError::SeparatorNotFound, "SeparatorNotFound"),
        ] {
            assert_eq!(
                to_value(error).expect("serializes"),
                json!({ "code": code })
            );
        }
        assert_eq!(
            to_value(ImportError::DatabaseError).expect("serializes"),
            json!({ "code": "DatabaseError" })
        );
    }
}
