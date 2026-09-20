//! Scheduled daily price download (SPF spec): downloads yesterday's closing
//! prices and exchange rates once per day, even while the app is closed, by
//! registering a native OS schedule (systemd/schtasks/launchd) that launches
//! the application invisibly.

/// Tauri command handlers (`configure_scheduled_fetch`, `get_scheduled_fetch_status`).
pub mod api;
/// Flat wire-facing error enum (`ScheduledFetchError`).
pub mod error;
/// Headless entry point invoked by `main.rs --scheduled-fetch` (SPF-016/020).
pub mod headless;
/// Orchestrator with `configure`, `status`, and `run_scheduled_fetch` methods.
pub mod orchestrator;
/// Use-case-owned persistence (`ScheduledFetchRepository`, configuration + run records).
pub mod repository;

pub use api::*;
pub use error::ScheduledFetchError;
pub use orchestrator::ScheduledFetchOrchestrator;
pub use repository::{
    ScheduledFetchConfiguration, ScheduledFetchOutcome, ScheduledFetchRepository,
    ScheduledFetchRun, ScheduledFetchStatus, SqliteScheduledFetchRepository,
};

#[cfg(test)]
pub use repository::MockScheduledFetchRepository;
