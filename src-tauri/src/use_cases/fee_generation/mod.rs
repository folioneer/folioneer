//! Fee Generation use case.
//!
//! Lazy catch-up orchestrator that generates management fee deductions for all
//! active schedules, covering every completed period since `last_applied_period`
//! (FEE-040/041/042/043/044/045/047/070).

/// Tauri command handler for `apply_due_fee_deductions`.
mod api;
/// Use-case-owned typed errors.
mod error;
/// Fee generation orchestrator.
pub mod orchestrator;

pub use api::*;
pub use error::FeeGenerationError;
pub use orchestrator::{FeeGenerationOrchestrator, LaunchSyncSurface};
