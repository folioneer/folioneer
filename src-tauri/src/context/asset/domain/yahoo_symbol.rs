/// Derives the Yahoo Finance provider symbol using the MKT-110 precedence rule
/// (ADR-017).
///
/// Precedence:
///   1. `exchange` is `Some` AND the mapper returns a suffix:
///      - non-empty suffix → `{BASE}.{suffix}` (e.g. `VOD.L`)
///      - empty suffix (US venues) → `{BASE}` (bare ticker, e.g. `AAPL`)
///   2. `exchange` is `None` → `{BASE}` (legacy / US happy path)
///   3. mapper returns `None` (exchange outside the table) → `None` (skip, MKT-114)
///
/// `BASE` is the trimmed reference with a class-share `/` translated to Yahoo's
/// `-` convention (`BRK/B` → `BRK-B`). Returns `None` for empty or non-ASCII
/// references regardless of the exchange.
pub fn derive_yahoo_symbol_with_exchange(
    reference: &str,
    exchange: Option<&super::exchange::Exchange>,
) -> Option<String> {
    let base = derive_yahoo_symbol(reference)?;
    match exchange {
        None => Some(base),
        Some(exchange) => {
            let suffix = super::yahoo_exchange_mapper::exchange_to_yahoo_suffix(exchange)?;
            if suffix.is_empty() {
                Some(base)
            } else {
                Some(format!("{base}.{suffix}"))
            }
        }
    }
}

/// Derives the base Yahoo symbol from an asset reference string (MKT-110).
///
/// A class-share `/` separator (OpenFIGI spells Berkshire B as `BRK/B`) is
/// translated to Yahoo's `-` convention (`BRK-B`); Yahoo addresses tickers in
/// upper case. Returns `None` (asset skipped, MKT-114) unless the result is a
/// clean ticker of `[A-Z0-9.-]` only — this allow-list rejects any reference
/// carrying URL path/query characters (`?`, `#`, `%`, `@`, …) so they cannot be
/// interpolated into the fetch URL.
pub fn derive_yahoo_symbol(reference: &str) -> Option<String> {
    let symbol = reference.trim().replace('/', "-").to_uppercase();
    if symbol.is_empty()
        || !symbol
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '-')
    {
        return None;
    }
    Some(symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xpar_exchange() -> crate::context::asset::Exchange {
        crate::context::asset::exchange::lookup("XPAR").expect("XPAR must be in the curated set")
    }

    fn xlon_exchange() -> crate::context::asset::Exchange {
        crate::context::asset::exchange::lookup("XLON").expect("XLON must be in the curated set")
    }

    fn xnas_exchange() -> crate::context::asset::Exchange {
        crate::context::asset::exchange::lookup("XNAS").expect("XNAS must be in the curated set")
    }

    fn non_curated_exchange() -> crate::context::asset::Exchange {
        crate::context::asset::Exchange {
            code: "XBOG".to_string(),
            label: "Bogus Exchange".to_string(),
        }
    }

    // MKT-110 — bare ticker uppercased (e.g. "aapl" -> "AAPL")
    #[test]
    fn bare_ticker_is_uppercased() {
        assert_eq!(derive_yahoo_symbol("aapl"), Some("AAPL".to_string()));
    }

    // MKT-110 — OpenFIGI class-share slash ("BRK/B") -> Yahoo hyphen ("BRK-B")
    #[test]
    fn class_share_slash_is_normalized_to_hyphen() {
        assert_eq!(derive_yahoo_symbol("BRK/B"), Some("BRK-B".to_string()));
    }

    // MKT-110 — empty / whitespace / non-ASCII references return None
    #[test]
    fn empty_reference_returns_none() {
        assert!(derive_yahoo_symbol("").is_none());
    }

    #[test]
    fn whitespace_only_reference_returns_none() {
        assert!(derive_yahoo_symbol("   ").is_none());
    }

    #[test]
    fn non_ascii_reference_returns_none() {
        assert!(derive_yahoo_symbol("日本電信電話").is_none());
    }

    // MKT-114 / hardening — a reference carrying URL path/query characters is
    // rejected (not interpolated into the fetch URL).
    #[test]
    fn url_injection_characters_return_none() {
        assert!(derive_yahoo_symbol("AAPL?interval=5m").is_none());
        assert!(derive_yahoo_symbol("AAPL#frag").is_none());
        assert!(derive_yahoo_symbol("e@vil").is_none());
        assert!(derive_yahoo_symbol("AA PL").is_none()); // embedded space
    }

    // MKT-110 step 1 — non-US exchange yields a venue-suffixed symbol
    #[test]
    fn with_exchange_returns_suffixed_symbol_for_non_us() {
        assert_eq!(
            derive_yahoo_symbol_with_exchange("VOD", Some(&xlon_exchange())),
            Some("VOD.L".to_string())
        );
    }

    #[test]
    fn with_exchange_paris_suffix() {
        assert_eq!(
            derive_yahoo_symbol_with_exchange("MC", Some(&xpar_exchange())),
            Some("MC.PA".to_string())
        );
    }

    // MKT-110 step 1 — US exchange (empty suffix) yields the bare ticker
    #[test]
    fn with_us_exchange_returns_bare_ticker() {
        assert_eq!(
            derive_yahoo_symbol_with_exchange("AAPL", Some(&xnas_exchange())),
            Some("AAPL".to_string())
        );
    }

    // MKT-110 step 2 — exchange absent → bare reference (legacy / US happy path)
    #[test]
    fn with_exchange_none_returns_bare_reference() {
        assert_eq!(
            derive_yahoo_symbol_with_exchange("AAPL", None),
            Some("AAPL".to_string())
        );
    }

    // MKT-110 step 3 — exchange set but mapper gap → None (asset skipped, MKT-114)
    #[test]
    fn with_exchange_mapper_gap_returns_none() {
        assert!(derive_yahoo_symbol_with_exchange("TICK", Some(&non_curated_exchange())).is_none());
    }

    // MKT-110 — empty / non-ASCII reference returns None regardless of exchange
    #[test]
    fn with_exchange_empty_reference_returns_none() {
        assert!(derive_yahoo_symbol_with_exchange("", Some(&xpar_exchange())).is_none());
    }
}
