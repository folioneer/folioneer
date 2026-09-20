//! Wire-facing error surface for the price freshness read (docs/error-model.md).

use serde::Serialize;
use specta::Type;

/// Flat error enum for `get_price_freshness`; serializes as `{ "code": "..." }`.
#[derive(Debug, thiserror::Error, Serialize, Type, Clone, PartialEq)]
#[serde(tag = "code")]
pub enum PriceFreshnessError {
    /// A read failed; the cause is logged server-side.
    #[error("An unexpected database error occurred")]
    DatabaseError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    // error-model.md wire-shape check — the variant emits a flat { "code": "..." }.
    #[test]
    fn database_error_emits_its_code() {
        assert_eq!(
            to_value(PriceFreshnessError::DatabaseError).unwrap(),
            json!({ "code": "DatabaseError" })
        );
    }
}
