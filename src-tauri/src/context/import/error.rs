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
            to_value(ImportError::DatabaseError).expect("serializes"),
            json!({ "code": "DatabaseError" })
        );
    }
}
