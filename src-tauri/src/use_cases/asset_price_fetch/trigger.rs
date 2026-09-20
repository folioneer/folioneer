//! PMV-010/015 — which action started an all-accounts price-fetch task. The
//! frontend states what the user did (a fact only it holds); the backend
//! decides what that means — only `Manual` produces a Price Movement report
//! (PMV-010). `fetch_account_asset_prices` needs no trigger: it never reports
//! movement.

use serde::Deserialize;
use specta::Type;

/// PMV-015 — exactly two variants. The Scheduled fetch (SPF) owns its own
/// sweep (`use_cases::scheduled_fetch::orchestrator`) and never reaches
/// `Dispatcher::spawn` or publishes `AssetPriceFetchCompleted`, so it needs no
/// third variant here.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Type)]
pub enum FetchTrigger {
    /// Auto-fetch on application launch (MKT-121/122). Never reports movement.
    Launch,
    /// Global refresh triggered by the user from the accounts list (MKT-130).
    /// The only trigger that produces a Price Movement report (PMV-010).
    Manual,
}

#[cfg(test)]
mod tests {
    use super::*;

    // PMV-015 — the wire deserialization shape matches the contract's bare
    // string variants ("Launch" | "Manual"), not a tagged object.
    #[test]
    fn deserializes_from_the_contract_string_variants() {
        let launch: FetchTrigger = serde_json::from_value(serde_json::json!("Launch")).unwrap();
        let manual: FetchTrigger = serde_json::from_value(serde_json::json!("Manual")).unwrap();
        assert_eq!(launch, FetchTrigger::Launch);
        assert_eq!(manual, FetchTrigger::Manual);
    }

    // PMV-015 — exactly two variants; a third value is rejected rather than
    // silently accepted, since no third trigger reaches this command (the
    // Scheduled fetch bypasses it entirely).
    #[test]
    fn rejects_an_unknown_variant() {
        let result: Result<FetchTrigger, _> =
            serde_json::from_value(serde_json::json!("Scheduled"));
        assert!(result.is_err());
    }
}
