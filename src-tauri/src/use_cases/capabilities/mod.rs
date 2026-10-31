//! What can run now, read by the interface (MKT-211).
//!
//! No orchestrator: there is nothing to coordinate. `api.rs` answers from the build's
//! channel, which the composition root managed, and from the External provider chosen on
//! this computer (MKT-214), which the interface reads again when it changes.

/// Tauri commands of the capabilities use case.
#[cfg(feature = "app")]
pub mod api;

#[cfg(feature = "app")]
pub use api::*;
