//! What this build can do, decided at composition and read by the interface (MKT-211).
//!
//! No orchestrator: there is nothing to coordinate. The answer is a property of how the
//! application was composed (`extensions.rs`, ADR-020), settled before the first window
//! opens and constant for the run, so `api.rs` returns the value the composition root
//! managed rather than computing one.

/// Tauri commands of the capabilities use case.
pub mod api;

pub use api::*;
