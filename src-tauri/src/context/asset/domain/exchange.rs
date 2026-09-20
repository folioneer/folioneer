/// Canonical trading venue value object for the asset bounded context.
///
/// `Exchange` is the single source of truth for all venue-related logic: the
/// `Asset` aggregate carries an optional reference to one, the Yahoo outbound
/// mapper resolves it to a provider suffix, and the OpenFIGI inbound mapper
/// resolves provider codes to it. Only venues in the hardcoded curated set are
/// considered canonical (AST-001).
use serde::{Deserialize, Serialize};
use specta::Type;

/// A canonical trading venue identified by its ISO 10383 MIC code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Exchange {
    /// ISO 10383 Market Identifier Code (e.g. "XPAR", "XNAS").
    pub code: String,
    /// Human-readable display name (e.g. "Euronext Paris").
    pub label: String,
}

/// Resolves a MIC code against the canonical curated set.
///
/// Returns a cloned `Exchange` when `code` matches a curated entry, `None`
/// otherwise. Used by `Asset::validate` (AST-001) and by the repository
/// read path to reconstruct the value object from the stored code.
pub fn lookup(code: &str) -> Option<Exchange> {
    CANONICAL_EXCHANGES
        .iter()
        .find(|(mic, _)| *mic == code)
        .map(|(mic, label)| Exchange {
            code: (*mic).to_string(),
            label: (*label).to_string(),
        })
}

/// Returns the full canonical curated set as owned values.
///
/// Consumed by the `get_supported_exchanges` Tauri command — the FE picker
/// source (AST-021). Treated as session-static: no refresh, no invalidation.
pub fn all() -> Vec<Exchange> {
    CANONICAL_EXCHANGES
        .iter()
        .map(|(mic, label)| Exchange {
            code: (*mic).to_string(),
            label: (*label).to_string(),
        })
        .collect()
}

/// The hardcoded curated set of supported trading venues (locked decision 1).
///
/// Intersection of OpenFIGI `micCode` coverage and Yahoo venue suffixes.
const CANONICAL_EXCHANGES: &[(&str, &str)] = &[
    ("XPAR", "Euronext Paris"),
    ("XNAS", "NASDAQ"),
    ("XNYS", "New York Stock Exchange"),
    ("XLON", "London Stock Exchange"),
    ("XETR", "Deutsche Börse Xetra"),
    ("XAMS", "Euronext Amsterdam"),
    ("XBRU", "Euronext Brussels"),
    ("XMIL", "Borsa Italiana"),
    ("XMAD", "Bolsas y Mercados Españoles"),
    ("XSWX", "SIX Swiss Exchange"),
    ("XTSE", "Toronto Stock Exchange"),
    ("XHKG", "Hong Kong Stock Exchange"),
    ("XTKS", "Tokyo Stock Exchange"),
    ("XASX", "Australian Securities Exchange"),
];

#[cfg(test)]
mod tests {
    use super::*;

    // all() returns a non-empty list
    #[test]
    fn all_returns_non_empty_curated_set() {
        let exchanges = all();
        assert!(
            !exchanges.is_empty(),
            "all() must return a non-empty curated set"
        );
    }

    // all() contains every MIC from the plan's locked starter list
    #[test]
    fn all_contains_expected_mics() {
        let exchanges = all();
        let codes: Vec<&str> = exchanges.iter().map(|e| e.code.as_str()).collect();
        let expected = [
            "XPAR", "XNAS", "XNYS", "XLON", "XETR", "XAMS", "XBRU", "XMIL", "XMAD", "XSWX", "XTSE",
            "XHKG", "XTKS", "XASX",
        ];
        for mic in &expected {
            assert!(
                codes.contains(mic),
                "curated set is missing expected MIC: {mic}"
            );
        }
    }

    // all() has no duplicate MIC codes
    #[test]
    fn all_has_no_duplicate_mic_codes() {
        let exchanges = all();
        let mut seen = std::collections::HashSet::new();
        for exchange in &exchanges {
            assert!(
                seen.insert(exchange.code.as_str()),
                "duplicate MIC code in curated set: {}",
                exchange.code
            );
        }
    }

    // every entry in all() has a non-empty label
    #[test]
    fn all_every_entry_has_non_empty_label() {
        let exchanges = all();
        for exchange in &exchanges {
            assert!(
                !exchange.label.trim().is_empty(),
                "Exchange {} has an empty label",
                exchange.code
            );
        }
    }

    // lookup returns Some for a known MIC
    #[test]
    fn lookup_returns_some_for_known_mic() {
        let result = lookup("XPAR");
        assert!(result.is_some(), "lookup(\"XPAR\") expected Some, got None");
        let exchange = result.unwrap();
        assert_eq!(exchange.code, "XPAR");
        assert!(!exchange.label.is_empty());
    }

    // lookup returns None for an unknown MIC
    #[test]
    fn lookup_returns_none_for_unknown_mic() {
        let result = lookup("BOGUS");
        assert!(
            result.is_none(),
            "lookup(\"BOGUS\") expected None, got: {result:?}"
        );
    }

    // lookup is case-sensitive: lowercase version of a canonical MIC returns None
    #[test]
    fn lookup_is_case_sensitive() {
        let result = lookup("xpar");
        assert!(
            result.is_none(),
            "lookup(\"xpar\") expected None (case-sensitive), got: {result:?}"
        );
    }

    // lookup returns cloned values — two calls produce equal but independent values
    #[test]
    fn lookup_returns_clone_of_canonical_entry() {
        let first = lookup("XNAS").expect("XNAS is in the curated set");
        let second = lookup("XNAS").expect("XNAS is in the curated set");
        assert_eq!(first, second);
    }

    // verify a few specific label values to catch mis-ordering or mis-labelling
    #[test]
    fn lookup_xpar_returns_euronext_paris() {
        let exchange = lookup("XPAR").expect("XPAR must be in the curated set");
        assert_eq!(exchange.code, "XPAR");
        assert!(
            exchange.label.to_lowercase().contains("paris")
                || exchange.label.to_lowercase().contains("euronext"),
            "XPAR label should reference Paris or Euronext, got: {}",
            exchange.label
        );
    }

    #[test]
    fn lookup_xnas_returns_nasdaq() {
        let exchange = lookup("XNAS").expect("XNAS must be in the curated set");
        assert!(
            exchange.label.to_lowercase().contains("nasdaq"),
            "XNAS label should reference NASDAQ, got: {}",
            exchange.label
        );
    }
}
