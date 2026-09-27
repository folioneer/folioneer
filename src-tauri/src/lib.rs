//! Folioneer Backend
//!
//! This crate provides the backend logic for managing financial portfolios,
//! including assets, accounts, and exchange rates.

#![deny(missing_docs)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), deny(clippy::panic))]
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]
#![cfg_attr(not(test), deny(clippy::unreachable))]
#![cfg_attr(not(test), deny(clippy::todo))]
#![cfg_attr(not(test), deny(clippy::unimplemented))]

use crate::context::account::AccountService;
use crate::context::asset::AssetService;
use crate::core::{SideEffectEventBus, BACKEND};
use anyhow::Context;
use std::{fs, sync::Arc};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// The Tauri shell: window, commands, updater (feature `app`).
#[cfg(feature = "app")]
mod app;
/// DDD Bounded Contexts
pub mod context;
/// Shared core utilities
pub mod core;
/// The one file a build differs by: external data sources and update channel (ADR-020)
mod extensions;
/// Cross-cutting infrastructure shared across bounded contexts (gold layout)
pub mod shared;
/// Application use cases
pub mod use_cases;
#[cfg(feature = "app")]
pub use app::run;

/// Global application state.
pub struct AppState {
    /// Database connection pool.
    pub db: Arc<core::Database>,
    /// System-wide event bus.
    pub event_bus: Arc<SideEffectEventBus>,
    /// Unified asset management service.
    pub asset_service: Arc<AssetService>,
    /// Account management service (owns account, holding, and transaction operations).
    pub account_service: Arc<AccountService>,
}

/// Headless entry for the OS-triggered scheduled run (SPF-016, SPF-020):
/// no Tauri builder, no window — runs the sweep and returns the exit code.
pub fn run_scheduled_fetch_headless() -> i32 {
    use use_cases::scheduled_fetch::headless::{self, HeadlessProviders};

    exit_code_on_new_runtime(headless::run(|| {
        extensions::providers().map(|providers| HeadlessProviders {
            price: providers.price,
            rate_history: providers.rate_history,
        })
    }))
}

/// Runs a headless entry to completion on a runtime of its own and returns its exit
/// code; 1 when no runtime can be started.
fn exit_code_on_new_runtime(entry: impl std::future::Future<Output = i32>) -> i32 {
    match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime.block_on(entry),
        Err(error) => {
            eprintln!("headless run: no async runtime: {error}");
            1
        }
    }
}

/// Initialize tracing with dual output: append to `app.log` and write to stderr.
pub(crate) fn initialize_tracing(log_dir: &std::path::Path) -> anyhow::Result<()> {
    let log_file = log_dir.join("app.log");

    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .with_context(|| "Failed to open log file")?;

    tracing_subscriber::registry()
        .with(fmt::layer().with_ansi(false).with_writer(file))
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .try_init()
        .with_context(|| "Failed to install the tracing subscriber")?;

    tracing::trace!(target: BACKEND, "Logging initialized. Log file: {}", log_file.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The core sets up its own logging (#047): the log file is created in the given folder,
    // and a folder that does not exist is an error, not a panic.
    // The headless entry returns the exit code its run produces (#047, SPF-020).
    #[test]
    fn a_headless_run_returns_its_exit_code() {
        assert_eq!(exit_code_on_new_runtime(async { 0 }), 0);
        assert_eq!(exit_code_on_new_runtime(async { 3 }), 3);
    }

    #[test]
    fn tracing_writes_to_app_log_and_refuses_a_missing_folder() {
        let missing = std::env::temp_dir()
            .join("folioneer-no-such-folder")
            .join("deeper");
        assert!(initialize_tracing(&missing).is_err());

        let dir = tempfile::tempdir().expect("temp dir");
        initialize_tracing(dir.path()).expect("tracing installed");
        assert!(dir.path().join("app.log").exists());
    }
}
