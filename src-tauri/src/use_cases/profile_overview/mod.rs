//! PFL-055 — every profile with its own figures, never added across profiles.

/// Tauri command handler for the overview.
#[cfg(feature = "app")]
mod api;
/// The orchestrator: each profile's total value and performance, computed within it.
mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use orchestrator::{ProfileOverview, ProfileOverviewRow, ProfileOverviewUseCase};
