/// Yahoo Finance outbound mapper: resolves a canonical `Exchange` to its Yahoo
/// venue suffix (MKT-110, ADR-017).
///
/// Pure adapter translating the asset BC's canonical `Exchange` value object to
/// the Yahoo Finance symbol-suffix scheme (`{ticker}.{suffix}`, e.g. `VOD.L`).
/// US venues (NYSE/Nasdaq) map to an **empty** suffix — Yahoo addresses US
/// listings by the bare ticker. `None` means the exchange has no Yahoo mapping
/// (mapper gap → asset skipped per MKT-114).
use crate::context::asset::Exchange;

/// Maps a canonical `Exchange` to its Yahoo venue suffix.
///
/// - `Some("")` — US venues (XNAS/XNYS): the symbol is the bare ticker.
/// - `Some(suffix)` — non-US venues: the symbol is `{ticker}.{suffix}`.
/// - `None` — exchange outside the mapping (mapper gap → asset skipped, MKT-114).
pub fn exchange_to_yahoo_suffix(exchange: &Exchange) -> Option<&'static str> {
    EXCHANGE_TO_YAHOO_SUFFIX
        .iter()
        .find(|(mic, _)| *mic == exchange.code)
        .map(|(_, suffix)| *suffix)
}

/// Canonical Exchange MIC → Yahoo venue suffix. Mirrors Yahoo's symbol scheme:
/// the symbol component is `{ticker}.{suffix}` (e.g. `VOD.L` on the LSE), or the
/// bare ticker for US venues (empty suffix).
const EXCHANGE_TO_YAHOO_SUFFIX: &[(&str, &str)] = &[
    ("XNAS", ""),
    ("XNYS", ""),
    ("XPAR", "PA"),
    ("XLON", "L"),
    ("XETR", "DE"),
    ("XAMS", "AS"),
    ("XBRU", "BR"),
    ("XMIL", "MI"),
    ("XMAD", "MC"),
    ("XSWX", "SW"),
    ("XTSE", "TO"),
    ("XHKG", "HK"),
    ("XTKS", "T"),
    ("XASX", "AX"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn make_exchange(code: &str) -> Exchange {
        Exchange {
            code: code.to_string(),
            label: format!("Test exchange {code}"),
        }
    }

    // XPAR maps to the Paris Yahoo suffix
    #[test]
    fn xpar_maps_to_pa() {
        assert_eq!(exchange_to_yahoo_suffix(&make_exchange("XPAR")), Some("PA"));
    }

    // XLON maps to the London Yahoo suffix
    #[test]
    fn xlon_maps_to_l() {
        assert_eq!(exchange_to_yahoo_suffix(&make_exchange("XLON")), Some("L"));
    }

    // XETR maps to the German Yahoo suffix
    #[test]
    fn xetr_maps_to_de() {
        assert_eq!(exchange_to_yahoo_suffix(&make_exchange("XETR")), Some("DE"));
    }

    // US venues map to an empty suffix (bare ticker), NOT None
    #[test]
    fn xnas_maps_to_empty_suffix() {
        assert_eq!(exchange_to_yahoo_suffix(&make_exchange("XNAS")), Some(""));
    }

    #[test]
    fn xnys_maps_to_empty_suffix() {
        assert_eq!(exchange_to_yahoo_suffix(&make_exchange("XNYS")), Some(""));
    }

    // every canonical exchange in the curated set has a Yahoo mapping
    #[test]
    fn all_canonical_exchanges_return_a_suffix() {
        use crate::context::asset::exchange::all;
        for exchange in &all() {
            assert!(
                exchange_to_yahoo_suffix(exchange).is_some(),
                "canonical exchange {} has no Yahoo suffix mapping",
                exchange.code
            );
        }
    }

    // an exchange not in the curated set returns None (mapper gap → skip)
    #[test]
    fn unknown_exchange_returns_none() {
        assert!(exchange_to_yahoo_suffix(&make_exchange("XBOG")).is_none());
    }
}
