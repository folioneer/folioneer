// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use tauri::State;

/// What the interface is told about this build: what it can do and which distribution
/// channel it belongs to (MKT-211, UPD-030). Settled
/// at composition and constant for the run.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Capabilities {
    /// Whether this build has an External provider. Without one no fetch task, no
    /// scheduled fetch and no price history backfill exists (MKT-210), and the
    /// interface offers none of them (MKT-212).
    pub external_provider: bool,
    /// The distribution channel this build belongs to, shown beside its version;
    /// `None` for the public build (UPD-030).
    pub distribution_channel: Option<String>,
}

impl Capabilities {
    /// The capabilities of a build composed with or without an External provider, and
    /// belonging to `distribution_channel`.
    pub fn of_build(has_external_provider: bool, distribution_channel: Option<&str>) -> Self {
        Self {
            external_provider: has_external_provider,
            distribution_channel: distribution_channel.map(str::to_string),
        }
    }
}

/// Reports what this build can do, so the interface renders only what it permits
/// (MKT-211). Infallible: the value is managed before any window exists.
#[tauri::command]
#[specta::specta]
pub fn get_capabilities(capabilities: State<'_, Capabilities>) -> Capabilities {
    capabilities.inner().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    // MKT-211, UPD-030 — the interface is told whether the build has an External
    // provider and which channel it belongs to.
    #[test]
    fn a_build_reports_its_provider_and_its_channel() {
        let private = Capabilities::of_build(true, Some("private"));
        assert!(private.external_provider);
        assert_eq!(private.distribution_channel.as_deref(), Some("private"));

        let public = Capabilities::of_build(false, None);
        assert!(!public.external_provider);
        assert_eq!(public.distribution_channel, None);
    }
}
