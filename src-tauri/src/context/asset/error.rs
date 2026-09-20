use serde::Serialize;
use specta::Type;

/// Single flat error enum for the `asset` bounded context (gold error model).
///
/// Every failure the BC can raise — asset / category / price value-object and
/// aggregate-invariant rejections from domain methods, plus service-layer
/// lookup / uniqueness / infrastructure translation — lives in this one type.
/// `#[serde(tag = "code")]` makes each variant serialize as
/// `{ "code": "VariantName", ...payload }` on the wire.
#[derive(Debug, thiserror::Error, Serialize, Type, Clone)]
#[serde(tag = "code")]
pub enum AssetError {
    // --- Asset aggregate / value-object validation ---
    /// Asset name is empty or whitespace-only.
    #[error("Asset name cannot be empty")]
    NameEmpty,
    /// Asset reference (ticker/ISIN) is empty or whitespace-only.
    #[error("Asset reference cannot be empty")]
    ReferenceEmpty,
    /// Risk level is outside the 1–5 range.
    #[error("Risk level must be between 1 and 5 (received: {received})")]
    InvalidRiskLevel {
        /// The rejected value the caller supplied.
        received: u8,
    },
    /// The currency string is not a valid ISO 4217 code.
    #[error("Invalid currency code: {currency}")]
    InvalidCurrency {
        /// The offending currency string the caller supplied.
        currency: String,
    },
    /// The asset is archived and cannot be edited, nor can its prices be mutated.
    #[error("Cannot modify an archived asset")]
    Archived,
    /// The asset is a system Cash Asset and cannot be edited, archived, unarchived, or deleted (CSH-016).
    #[error("Cannot edit a system Cash Asset")]
    CashAssetNotEditable,
    /// The supplied exchange code is not a member of the canonical curated set (AST-001).
    #[error("Invalid exchange code: {exchange_code}")]
    InvalidExchange {
        /// The MIC code the caller supplied. Named `exchange_code` (not `code`) to avoid
        /// a conflict with the `#[serde(tag = "code")]` discriminant field.
        exchange_code: String,
    },
    /// The supplied ISIN fails the ISO 6166 format validation (AST-023, WEB-016).
    /// Sub-variants of `IsinFormatError` (wrong length, invalid charset, bad
    /// check digit) collapse to this single wire code.
    #[error("Invalid ISIN format")]
    InvalidIsinFormat,

    // --- AssetPrice value-object validation ---
    /// Price must be strictly positive.
    #[error("Price must be strictly positive")]
    NotPositive,
    /// Price value is not a finite floating-point number.
    #[error("Price must be a finite number")]
    NonFinite,
    /// Price date is in the future.
    #[error("Date cannot be in the future")]
    DateInFuture,
    /// The supplied date string is not parseable as ISO 8601 (`YYYY-MM-DD`).
    #[error("Invalid date format — expected YYYY-MM-DD (received: {date})")]
    InvalidDateFormat {
        /// The offending date string the caller supplied.
        date: String,
    },

    // --- Category aggregate / value-object validation ---
    /// Category label is empty or whitespace-only.
    #[error("Category label cannot be empty")]
    LabelEmpty,
    /// Attempt to rename the system default category.
    #[error("The system category cannot be renamed")]
    SystemReadonly,
    /// Attempt to delete the system default category.
    #[error("The system category cannot be deleted")]
    SystemProtected,

    // --- Service-layer lookup / uniqueness ---
    /// No asset exists with the requested ID. Born at the service layer when
    /// `asset_repo.get_by_id` returns `None`.
    #[error("Asset not found: {id}")]
    AssetNotFound {
        /// The ID the caller asked for.
        id: String,
    },
    /// No category exists with the requested ID. Born at the service layer when
    /// `category_repo.get_by_id` returns `None`.
    #[error("Category not found: {id}")]
    CategoryNotFound {
        /// The ID the caller asked for.
        id: String,
    },
    /// A category with the same name (case-insensitive) already exists. Born at
    /// the service layer from a `find_by_name` uniqueness pre-check.
    #[error("A category with this name already exists")]
    DuplicateName,
    /// No price record exists for the given (asset_id, date) pair (MKT-083 / MKT-090).
    #[error("Asset price not found for {asset_id} on {date}")]
    PriceNotFound {
        /// Asset whose price was being addressed.
        asset_id: String,
        /// Date the caller asked to update or delete.
        date: String,
    },

    // --- Infrastructure translation ---
    /// Application-layer translation of any infrastructure failure from an
    /// asset-side repository call. No `hint` payload on the wire; the full
    /// diagnostic chain is preserved server-side via `tracing::error!` at the
    /// translation site. FE shows the i18n key `error.DatabaseError`.
    #[error("An unexpected database error occurred")]
    DatabaseError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    /// Verifies the `#[serde(tag = "code")]` contract: every variant emits a
    /// flat `{ "code": "VariantName", ...payload }` object on the wire.
    #[test]
    fn each_variant_emits_a_code() {
        assert_eq!(
            to_value(AssetError::NameEmpty).unwrap(),
            json!({ "code": "NameEmpty" })
        );
        assert_eq!(
            to_value(AssetError::ReferenceEmpty).unwrap(),
            json!({ "code": "ReferenceEmpty" })
        );
        assert_eq!(
            to_value(AssetError::InvalidRiskLevel { received: 6 }).unwrap(),
            json!({ "code": "InvalidRiskLevel", "received": 6 })
        );
        assert_eq!(
            to_value(AssetError::InvalidCurrency {
                currency: "XX".into()
            })
            .unwrap(),
            json!({ "code": "InvalidCurrency", "currency": "XX" })
        );
        assert_eq!(
            to_value(AssetError::Archived).unwrap(),
            json!({ "code": "Archived" })
        );
        assert_eq!(
            to_value(AssetError::CashAssetNotEditable).unwrap(),
            json!({ "code": "CashAssetNotEditable" })
        );
        assert_eq!(
            to_value(AssetError::InvalidExchange {
                exchange_code: "XXXX".into()
            })
            .unwrap(),
            json!({ "code": "InvalidExchange", "exchange_code": "XXXX" })
        );
        assert_eq!(
            to_value(AssetError::InvalidIsinFormat).unwrap(),
            json!({ "code": "InvalidIsinFormat" })
        );
        assert_eq!(
            to_value(AssetError::NotPositive).unwrap(),
            json!({ "code": "NotPositive" })
        );
        assert_eq!(
            to_value(AssetError::NonFinite).unwrap(),
            json!({ "code": "NonFinite" })
        );
        assert_eq!(
            to_value(AssetError::DateInFuture).unwrap(),
            json!({ "code": "DateInFuture" })
        );
        assert_eq!(
            to_value(AssetError::InvalidDateFormat { date: "bad".into() }).unwrap(),
            json!({ "code": "InvalidDateFormat", "date": "bad" })
        );
        assert_eq!(
            to_value(AssetError::LabelEmpty).unwrap(),
            json!({ "code": "LabelEmpty" })
        );
        assert_eq!(
            to_value(AssetError::SystemReadonly).unwrap(),
            json!({ "code": "SystemReadonly" })
        );
        assert_eq!(
            to_value(AssetError::SystemProtected).unwrap(),
            json!({ "code": "SystemProtected" })
        );
        assert_eq!(
            to_value(AssetError::AssetNotFound {
                id: "asset-1".into()
            })
            .unwrap(),
            json!({ "code": "AssetNotFound", "id": "asset-1" })
        );
        assert_eq!(
            to_value(AssetError::CategoryNotFound { id: "cat-1".into() }).unwrap(),
            json!({ "code": "CategoryNotFound", "id": "cat-1" })
        );
        assert_eq!(
            to_value(AssetError::DuplicateName).unwrap(),
            json!({ "code": "DuplicateName" })
        );
        assert_eq!(
            to_value(AssetError::PriceNotFound {
                asset_id: "asset-1".into(),
                date: "2026-01-01".into()
            })
            .unwrap(),
            json!({ "code": "PriceNotFound", "asset_id": "asset-1", "date": "2026-01-01" })
        );
        assert_eq!(
            to_value(AssetError::DatabaseError).unwrap(),
            json!({ "code": "DatabaseError" })
        );
    }
}
