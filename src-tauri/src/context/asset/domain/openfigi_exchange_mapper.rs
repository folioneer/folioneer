/// OpenFIGI inbound mapper: resolves OpenFIGI venue identifiers to canonical
/// `Exchange` values (WEB-049).
///
/// OpenFIGI responses carry two venue fields:
/// - `micCode` (ISO 10383 MIC, present only for some entries) — primary signal
/// - `exchCode` (OpenFIGI short code, e.g. "FP", "UN") — fallback signal
///
/// The WEB-049 precedence rule: consult `micCode` when present, otherwise fall
/// back to `exchCode` → MIC → `Exchange::lookup`. Returns `None` when neither
/// resolves into the canonical set (venue outside the curated list).
use crate::context::asset::{exchange, Exchange};

/// Resolves an OpenFIGI MIC code directly to a canonical `Exchange`.
///
/// Returns `Some(Exchange)` when `mic` is a member of the curated set, `None`
/// otherwise. This is the primary resolution path per WEB-049.
pub fn openfigi_mic_to_exchange(mic: &str) -> Option<Exchange> {
    exchange::lookup(mic)
}

/// Resolves an OpenFIGI `exchCode` short code to a canonical `Exchange`.
///
/// First maps the short code to its ISO 10383 MIC via a hardcoded table, then
/// delegates to `exchange::lookup`. Returns `None` when the code is unknown or
/// when the resolved MIC is not in the curated set.
///
/// Used as the fallback when `micCode` is absent from the OpenFIGI hit.
pub fn openfigi_exchcode_to_exchange(exch_code: &str) -> Option<Exchange> {
    EXCHCODE_TO_MIC
        .iter()
        .find(|(code, _)| *code == exch_code)
        .and_then(|(_, mic)| exchange::lookup(mic))
}

/// OpenFIGI `exchCode` → ISO 10383 MIC. Only covers the codes that map to a
/// venue in the canonical curated set. Unknown codes are not represented.
///
/// Some venues are exposed under two OpenFIGI short codes; both must be mapped
/// or the priority walker can pick a code that doesn't resolve, leaving the
/// result with no exchange label. `LO`/`LN` and `SQ`/`SM` are the known pairs
/// in the curated set today.
///
/// Source: OpenFIGI Exchange Codes CSV cross-referenced against the curated set.
const EXCHCODE_TO_MIC: &[(&str, &str)] = &[
    ("FP", "XPAR"),
    ("UN", "XNYS"),
    ("UW", "XNAS"),
    ("LN", "XLON"),
    ("LO", "XLON"),
    ("GY", "XETR"),
    ("NA", "XAMS"),
    ("BB", "XBRU"),
    ("IM", "XMIL"),
    ("SM", "XMAD"),
    ("SQ", "XMAD"),
    ("SE", "XSWX"),
    ("CT", "XTSE"),
    ("HK", "XHKG"),
    ("JT", "XTKS"),
    ("AT", "XASX"),
];

#[cfg(test)]
mod tests {
    use super::*;

    // openfigi_mic_to_exchange — known MIC in the curated set
    #[test]
    fn mic_to_exchange_returns_some_for_canonical_mic() {
        let result = openfigi_mic_to_exchange("XPAR");
        assert!(
            result.is_some(),
            "openfigi_mic_to_exchange(\"XPAR\") expected Some, got None"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XPAR");
        assert!(!exchange.label.is_empty());
    }

    // openfigi_mic_to_exchange — unknown MIC returns None
    #[test]
    fn mic_to_exchange_returns_none_for_unknown_mic() {
        let result = openfigi_mic_to_exchange("XBOG");
        assert!(
            result.is_none(),
            "openfigi_mic_to_exchange(\"XBOG\") expected None (not in curated set), got: {result:?}"
        );
    }

    // openfigi_mic_to_exchange — empty string returns None
    #[test]
    fn mic_to_exchange_returns_none_for_empty_string() {
        let result = openfigi_mic_to_exchange("");
        assert!(
            result.is_none(),
            "openfigi_mic_to_exchange(\"\") expected None, got: {result:?}"
        );
    }

    // openfigi_exchcode_to_exchange — known OpenFIGI exch_code for Paris
    #[test]
    fn exchcode_to_exchange_fp_returns_xpar() {
        let result = openfigi_exchcode_to_exchange("FP");
        assert!(
            result.is_some(),
            "openfigi_exchcode_to_exchange(\"FP\") expected Some(XPAR), got None"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XPAR");
    }

    // openfigi_exchcode_to_exchange — known OpenFIGI exch_code for NYSE
    #[test]
    fn exchcode_to_exchange_un_returns_xnys() {
        let result = openfigi_exchcode_to_exchange("UN");
        assert!(
            result.is_some(),
            "openfigi_exchcode_to_exchange(\"UN\") expected Some(XNYS), got None"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XNYS");
    }

    // openfigi_exchcode_to_exchange — known OpenFIGI exch_code for NASDAQ
    #[test]
    fn exchcode_to_exchange_uw_returns_xnas() {
        let result = openfigi_exchcode_to_exchange("UW");
        assert!(
            result.is_some(),
            "openfigi_exchcode_to_exchange(\"UW\") expected Some(XNAS), got None"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XNAS");
    }

    // openfigi_exchcode_to_exchange — unknown code returns None
    #[test]
    fn exchcode_to_exchange_returns_none_for_unknown_code() {
        let result = openfigi_exchcode_to_exchange("ZZ");
        assert!(
            result.is_none(),
            "openfigi_exchcode_to_exchange(\"ZZ\") expected None, got: {result:?}"
        );
    }

    // openfigi_exchcode_to_exchange — empty code returns None
    #[test]
    fn exchcode_to_exchange_returns_none_for_empty_code() {
        let result = openfigi_exchcode_to_exchange("");
        assert!(
            result.is_none(),
            "openfigi_exchcode_to_exchange(\"\") expected None, got: {result:?}"
        );
    }

    // openfigi_exchcode_to_exchange — known OpenFIGI exch_code for Xetra
    #[test]
    fn exchcode_to_exchange_gy_returns_xetr() {
        let result = openfigi_exchcode_to_exchange("GY");
        assert!(
            result.is_some(),
            "openfigi_exchcode_to_exchange(\"GY\") expected Some(XETR), got None"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XETR");
    }

    // OpenFIGI exposes LSE under both LN (lit primary) and LO (consolidated
    // listing). Both must resolve to XLON or the priority walker can pick LO
    // and leave the result with no exchange label.
    #[test]
    fn exchcode_to_exchange_lo_returns_xlon() {
        let result = openfigi_exchcode_to_exchange("LO");
        assert!(
            result.is_some(),
            "openfigi_exchcode_to_exchange(\"LO\") expected Some(XLON), got None"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XLON");
    }

    // OpenFIGI exposes Madrid under both SM (Continuous Market) and SQ (BME
    // composite). The ES ISIN-country promotion in primary_listing_processor
    // picks SQ first, so SQ must resolve to XMAD.
    #[test]
    fn exchcode_to_exchange_sq_returns_xmad() {
        let result = openfigi_exchcode_to_exchange("SQ");
        assert!(
            result.is_some(),
            "openfigi_exchcode_to_exchange(\"SQ\") expected Some(XMAD), got None"
        );
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XMAD");
    }
}
