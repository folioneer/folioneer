/// Single flat error enum for the `import` bounded context (gold error model).
///
/// `#[serde(tag = "code")]` makes each variant serialize as `{ "code": "VariantName", ... }`
/// on the wire.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum ImportError {
    /// The mapper's provider is empty once trimmed (IMP-024).
    #[error("An import mapper needs a provider")]
    MapperProviderEmpty,

    /// Another mapper holds the provider, case and accents ignored (IMP-024).
    #[error("Another import mapper is of the provider {provider}")]
    MapperProviderTaken {
        /// The provider asked for.
        provider: String,
    },

    /// The mapper names no date column, no operation column, or no asset column (IMP-021).
    #[error("An import mapper needs the date, the operation and an asset column")]
    MapperColumnMissing,

    /// A text of the mapper is longer than `MAPPER_TEXT_LENGTH` characters (IMP-024A).
    #[error("A text of the import mapper is too long")]
    MapperTextTooLong,

    /// The mapper is no longer held (IMP-020, IMP-025, IMP-040).
    #[error("Import mapper not found: {mapper_id}")]
    MapperNotFound {
        /// The mapper asked for.
        mapper_id: String,
    },

    /// The mapper's file cannot be read (IMP-020B).
    #[error("The import mapper's file cannot be read")]
    MapperFileUnreadable,

    /// The mapper's file is larger than `LARGEST_MAPPER_FILE` (IMP-020B).
    #[error("The import mapper's file is larger than 1 MB")]
    MapperFileTooLarge,

    /// The file does not read as a mapper (IMP-020B).
    #[error("The file does not read as an import mapper: {reason}")]
    MapperFileInvalid {
        /// Why, as the reader says it: what is wrong and where.
        reason: String,
    },

    /// The mappers folder cannot be read or written; the cause is logged, never sent.
    #[error("The import mappers folder cannot be read or written")]
    MapperFolderFailed,

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

    /// The mapper in use names a column the file lacks (IMP-021).
    #[error("The file has no column {column}")]
    MappedColumnNotInFile {
        /// The column's header, as the mapper names it.
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
            to_value(ImportError::MapperProviderEmpty).expect("serializes"),
            json!({ "code": "MapperProviderEmpty" })
        );
        assert_eq!(
            to_value(ImportError::MapperProviderTaken {
                provider: "PEA".into()
            })
            .expect("serializes"),
            json!({ "code": "MapperProviderTaken", "provider": "PEA" })
        );
        assert_eq!(
            to_value(ImportError::MapperColumnMissing).expect("serializes"),
            json!({ "code": "MapperColumnMissing" })
        );
        assert_eq!(
            to_value(ImportError::MapperTextTooLong).expect("serializes"),
            json!({ "code": "MapperTextTooLong" })
        );
        assert_eq!(
            to_value(ImportError::MapperNotFound {
                mapper_id: "m-1".into()
            })
            .expect("serializes"),
            json!({ "code": "MapperNotFound", "mapper_id": "m-1" })
        );
        assert_eq!(
            to_value(ImportError::MapperFileInvalid {
                reason: "line 1".into()
            })
            .expect("serializes"),
            json!({ "code": "MapperFileInvalid", "reason": "line 1" })
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
        for (error, code) in [
            (ImportError::MapperFileUnreadable, "MapperFileUnreadable"),
            (ImportError::MapperFileTooLarge, "MapperFileTooLarge"),
            (ImportError::MapperFolderFailed, "MapperFolderFailed"),
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
