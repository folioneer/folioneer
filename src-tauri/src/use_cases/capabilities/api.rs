// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use crate::use_cases::shared::chosen_price_provider::ChosenPriceProvider;

/// What the interface is told: whether this computer has an External provider chosen, and
/// which distribution channel the build belongs to (MKT-211, UPD-030).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Capabilities {
    /// Whether an External provider is chosen on this computer. Without one no fetch task,
    /// no scheduled fetch and no price history backfill runs (MKT-210), and the interface
    /// offers none of them (MKT-212).
    pub external_provider: bool,
    /// The distribution channel this build belongs to, shown beside its version;
    /// `None` for the public build (UPD-030).
    pub distribution_channel: Option<String>,
}

impl Capabilities {
    /// The answer with or without an External provider chosen, for a build belonging to
    /// `distribution_channel`.
    pub fn of(has_external_provider: bool, distribution_channel: Option<&str>) -> Self {
        Self {
            external_provider: has_external_provider,
            distribution_channel: distribution_channel.map(str::to_string),
        }
    }
}

/// Reports what can run now, so the interface renders only what it permits (MKT-211).
/// Infallible: both values are managed before any window exists.
#[tauri::command]
#[specta::specta]
pub fn get_capabilities(
    capabilities: State<'_, Capabilities>,
    chosen: State<'_, Arc<ChosenPriceProvider>>,
) -> Capabilities {
    Capabilities::of(chosen.is_on(), capabilities.distribution_channel.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    // MKT-211, UPD-030 — the interface is told whether an External provider is chosen and
    // which channel the build belongs to.
    #[test]
    fn a_build_reports_its_provider_and_its_channel() {
        let private = Capabilities::of(true, Some("private"));
        assert!(private.external_provider);
        assert_eq!(private.distribution_channel.as_deref(), Some("private"));

        let public = Capabilities::of(false, None);
        assert!(!public.external_provider);
        assert_eq!(public.distribution_channel, None);
    }
}
