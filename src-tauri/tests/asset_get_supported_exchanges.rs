/// Integration tests for the `get_supported_exchanges` Tauri command (AST-021).
///
/// These tests exercise the public API: `exchange::all()` is the domain constant
/// exposed by the infallible `get_supported_exchanges` handler. The command
/// itself has no service or repository involvement — it is a thin pass-through
/// to the domain constant (per locked decision 2 in the plan).
///
/// Tests here verify:
/// - The public `exchange::all()` function is accessible via the crate's public API.
/// - The returned set is non-empty and matches the curated constant.
/// - The returned set contains the 14 MICs committed in the plan.
/// - No duplicate codes are present.
///
/// The command handler itself (the Tauri `#[tauri::command]` fn) is verified at
/// compile time by specta_builder; wiring is confirmed here at the domain level
/// because the handler is a one-liner (`exchange::all()`).
use folioneer_lib::context::asset::exchange;

/// get_supported_exchanges — happy path: returns the canonical curated set.
#[test]
fn get_supported_exchanges_returns_non_empty_set() {
    let exchanges = exchange::all();
    assert!(
        !exchanges.is_empty(),
        "get_supported_exchanges must return a non-empty set"
    );
}

/// get_supported_exchanges — count matches the plan's locked starter list of 14 venues.
#[test]
fn get_supported_exchanges_count_is_at_least_14() {
    let exchanges = exchange::all();
    assert!(
        exchanges.len() >= 14,
        "get_supported_exchanges must contain at least the 14 plan-locked venues, got: {}",
        exchanges.len()
    );
}

/// get_supported_exchanges — contains every MIC from the plan's locked starter list.
#[test]
fn get_supported_exchanges_contains_all_plan_locked_mics() {
    let exchanges = exchange::all();
    let codes: Vec<&str> = exchanges.iter().map(|e| e.code.as_str()).collect();
    let required = [
        "XPAR", "XNAS", "XNYS", "XLON", "XETR", "XAMS", "XBRU", "XMIL", "XMAD", "XSWX", "XTSE",
        "XHKG", "XTKS", "XASX",
    ];
    for mic in &required {
        assert!(
            codes.contains(mic),
            "get_supported_exchanges result is missing required MIC: {mic}"
        );
    }
}

/// get_supported_exchanges — no duplicate MIC codes.
#[test]
fn get_supported_exchanges_has_no_duplicate_codes() {
    let exchanges = exchange::all();
    let mut seen = std::collections::HashSet::new();
    for exchange in &exchanges {
        assert!(
            seen.insert(exchange.code.as_str()),
            "get_supported_exchanges returned a duplicate MIC code: {}",
            exchange.code
        );
    }
}

/// get_supported_exchanges — every entry has a non-empty label.
#[test]
fn get_supported_exchanges_every_entry_has_non_empty_label() {
    let exchanges = exchange::all();
    for exchange in &exchanges {
        assert!(
            !exchange.label.trim().is_empty(),
            "exchange {} has an empty label in the supported set",
            exchange.code
        );
    }
}

/// get_supported_exchanges — result matches exchange::lookup for each code.
///
/// Verifies that the curated set and the lookup function are consistent: every
/// code returned by `all()` must be resolvable by `lookup()`.
#[test]
fn get_supported_exchanges_every_code_resolves_via_lookup() {
    let exchanges = exchange::all();
    for exchange in &exchanges {
        let resolved = exchange::lookup(&exchange.code);
        assert!(
            resolved.is_some(),
            "exchange::lookup({}) returned None but {} was returned by all()",
            exchange.code,
            exchange.code
        );
        let resolved = resolved.unwrap();
        assert_eq!(
            resolved.code, exchange.code,
            "lookup({}) returned a different code than expected",
            exchange.code
        );
    }
}
