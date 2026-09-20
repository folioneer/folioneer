/// Single flat error enum for the `currency` bounded context (gold error model).
///
/// Every failure the BC can raise — validation, lookup, and infrastructure —
/// lives in this one type. `#[serde(tag = "code")]` makes each variant
/// serialize as `{ "code": "VariantName", ...payload }` on the wire.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum CurrencyError {
    /// The rate value is zero or negative (FXR-021).
    #[error("Rate must be strictly positive")]
    NotPositive,

    /// The rate value is not a finite floating-point number (FXR-021). Raised
    /// only at the IPC boundary by `api::rate_f64_to_micros`, before the `f64`
    /// is converted to the i64 micros the domain factory accepts.
    #[error("Rate must be a finite number")]
    NonFinite,

    /// The supplied date is in the future (FXR-022).
    #[error("Date cannot be in the future")]
    DateInFuture,

    /// The supplied date string is not parseable as ISO 8601 `YYYY-MM-DD` (FXR-022).
    #[error("Invalid date format — expected YYYY-MM-DD (received: {date})")]
    InvalidDateFormat {
        /// The offending date string.
        date: String,
    },

    /// The supplied currency code is not a recognised ISO 4217 code (FXR-023).
    #[error("Invalid currency code: {currency}")]
    InvalidCurrency {
        /// The offending currency string.
        currency: String,
    },

    /// Both sides of a pair are the same currency — the identity rate is
    /// implicit and is never stored (FXR-011/023).
    #[error("from_currency and to_currency must differ")]
    IdentityPair,

    /// No rate exists for the given pair on the given date (FXR-052/053).
    #[error("Rate not found: {from_currency}/{to_currency} on {date}")]
    RateNotFound {
        /// Source currency of the missing rate.
        from_currency: String,
        /// Target currency of the missing rate.
        to_currency: String,
        /// Date of the missing rate.
        date: String,
    },

    /// The external rate provider could not be reached at all during a
    /// user-triggered history backfill (FXR-114). The piggybacked fetch paths
    /// never raise this — they degrade silently (FXR-073).
    #[error("The exchange-rate provider could not be reached")]
    ProviderUnreachable,

    /// An infrastructure / database failure occurred. The full diagnostic is
    /// preserved server-side via `tracing::error!`; the wire surface carries
    /// no hint.
    #[error("An unexpected database error occurred")]
    DatabaseError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    /// Verifies the `#[serde(tag = "code")]` contract: every variant emits a
    /// flat `{ "code": "VariantName", ...payload }` object on the wire.
    /// A missing or mis-tagged variant collapses to `null` under an untagged
    /// composite — this test catches that regression for `CurrencyError` itself.
    #[test]
    fn each_variant_emits_a_code() {
        assert_eq!(
            to_value(CurrencyError::NotPositive).unwrap(),
            json!({ "code": "NotPositive" })
        );
        assert_eq!(
            to_value(CurrencyError::NonFinite).unwrap(),
            json!({ "code": "NonFinite" })
        );
        assert_eq!(
            to_value(CurrencyError::DateInFuture).unwrap(),
            json!({ "code": "DateInFuture" })
        );
        assert_eq!(
            to_value(CurrencyError::InvalidDateFormat {
                date: "not-a-date".into()
            })
            .unwrap(),
            json!({ "code": "InvalidDateFormat", "date": "not-a-date" })
        );
        assert_eq!(
            to_value(CurrencyError::InvalidCurrency {
                currency: "XX".into()
            })
            .unwrap(),
            json!({ "code": "InvalidCurrency", "currency": "XX" })
        );
        assert_eq!(
            to_value(CurrencyError::ProviderUnreachable).unwrap(),
            json!({ "code": "ProviderUnreachable" })
        );
        assert_eq!(
            to_value(CurrencyError::IdentityPair).unwrap(),
            json!({ "code": "IdentityPair" })
        );
        assert_eq!(
            to_value(CurrencyError::RateNotFound {
                from_currency: "USD".into(),
                to_currency: "EUR".into(),
                date: "2026-01-01".into(),
            })
            .unwrap(),
            json!({
                "code": "RateNotFound",
                "from_currency": "USD",
                "to_currency": "EUR",
                "date": "2026-01-01"
            })
        );
        assert_eq!(
            to_value(CurrencyError::DatabaseError).unwrap(),
            json!({ "code": "DatabaseError" })
        );
    }
}
