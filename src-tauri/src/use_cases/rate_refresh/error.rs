use serde::Serialize;
use specta::Type;

/// Flat wire-facing error enum for `refresh_currency_rates` (FXR-075). A provider
/// failure is never an error here: the launch refresh is silent (FXR-070/073).
#[derive(Debug, thiserror::Error, Serialize, Type, Clone, PartialEq)]
#[serde(tag = "code")]
pub enum RateRefreshError {
    /// An unexpected database error occurred.
    #[error("An unexpected database error occurred")]
    DatabaseError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    // error-model.md wire-shape check — every variant emits a flat { "code": "..." }.
    #[test]
    fn each_variant_emits_a_code() {
        assert_eq!(
            to_value(RateRefreshError::DatabaseError).unwrap(),
            json!({ "code": "DatabaseError" })
        );
    }
}
