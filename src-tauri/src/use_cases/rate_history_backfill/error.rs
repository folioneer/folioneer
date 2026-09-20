use serde::Serialize;
use specta::Type;

/// Flat wire-facing error enum for `backfill_currency_rate_history`
/// (FXR-110/114).
#[derive(Debug, thiserror::Error, Serialize, Type, Clone, PartialEq)]
#[serde(tag = "code")]
pub enum RateHistoryBackfillError {
    /// The external rate provider could not be reached at all (FXR-114).
    #[error("The exchange-rate provider could not be reached")]
    ProviderUnreachable,
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
            to_value(RateHistoryBackfillError::ProviderUnreachable).unwrap(),
            json!({ "code": "ProviderUnreachable" })
        );
        assert_eq!(
            to_value(RateHistoryBackfillError::DatabaseError).unwrap(),
            json!({ "code": "DatabaseError" })
        );
    }
}
