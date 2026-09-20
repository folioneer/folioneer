/// Application-layer errors raised by the asset web-lookup use case (WEB-025).
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone)]
#[serde(tag = "code")]
pub enum WebLookupError {
    /// OpenFIGI returned HTTP 429 Too Many Requests — transient, recoverable
    /// after a short wait. Surfaced distinctly so the frontend can render
    /// retry-after-wait copy (WEB-033).
    #[error("Lookup service rate limit reached — wait a moment and retry")]
    RateLimited,
    /// Network unreachable, connection timeout, or any non-2xx HTTP status
    /// other than 429.
    #[error("Network error while contacting the lookup service")]
    NetworkError,
    /// The query was submitted on the ISIN path (`LookupMode::Isin`) but does
    /// not satisfy the ISO 6166 format rules: wrong length, invalid charset, or
    /// failing Luhn-mod-10 check digit (WEB-016, WEB-025).
    ///
    /// Wire shape: `{ "code": "InvalidIsinFormat" }` — no payload; the FE
    /// renders a single static copy string.
    #[error("The query is not a valid ISIN (WEB-016)")]
    InvalidIsinFormat,
}

// ---------------------------------------------------------------------------
// Serialization tests — wire shape (WEB-025)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn to_json(err: &WebLookupError) -> serde_json::Value {
        serde_json::to_value(err).expect("serialize")
    }

    #[test]
    fn serializes_rate_limited() {
        assert_eq!(
            to_json(&WebLookupError::RateLimited),
            serde_json::json!({ "code": "RateLimited" })
        );
    }

    #[test]
    fn serializes_network_error() {
        assert_eq!(
            to_json(&WebLookupError::NetworkError),
            serde_json::json!({ "code": "NetworkError" })
        );
    }

    #[test]
    fn serializes_invalid_isin_format() {
        assert_eq!(
            to_json(&WebLookupError::InvalidIsinFormat),
            serde_json::json!({ "code": "InvalidIsinFormat" })
        );
    }
}
