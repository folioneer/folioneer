// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use tauri::State;

/// What this build can do (MKT-211). Settled at composition and constant for the run.
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
pub struct Capabilities {
    /// Whether this build has an External provider. Without one no fetch task, no
    /// scheduled fetch and no price history backfill exists (MKT-210), and the
    /// interface offers none of them (MKT-212).
    pub external_provider: bool,
}

/// Reports what this build can do, so the interface renders only what it permits
/// (MKT-211). Infallible: the value is managed before any window exists.
#[tauri::command]
#[specta::specta]
pub fn get_capabilities(capabilities: State<'_, Capabilities>) -> Capabilities {
    *capabilities.inner()
}
