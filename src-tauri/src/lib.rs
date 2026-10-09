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

/// The agent connection: an interface beside the window, reached through a bridge (AGT).
mod agent_connection;
/// The Tauri shell: window, commands, updater (feature `app`).
#[cfg(feature = "app")]
mod app;
/// The command line: an interface beside the window, calling the same use cases (CLI).
mod command_line;
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

/// Headless entry for the command line (CLI spec, TODO-044): reads the arguments after the
/// program name, runs the command without a window, prints its result and returns the exit
/// code (CLI-022). Its log lines go to the log file only, never to the terminal.
pub fn run_command_line(program: &str, args: &[String]) -> i32 {
    // Logging is best-effort and never printed: the terminal shows only the command's result.
    if let Some(log_dir) = shared::infrastructure::app_directories::resolve_log_dir() {
        let _ = fs::create_dir_all(&log_dir)
            .map_err(anyhow::Error::from)
            .and_then(|()| initialize_tracing_to(&log_dir, false));
    }
    execute_command_line(program, args)
}

/// AGT-012 — whether the arguments start the bridge an agent talks to: `--mcp`, first.
pub fn starts_agent_bridge(args: &[String]) -> bool {
    args.first().is_some_and(|first| first == "--mcp")
}

/// Headless entry of the bridge (ADR-023, AGT-010): serves one agent client on the standard
/// input and output until it leaves, passing its tool calls to the running application. It
/// opens no database and writes no log: it holds nothing of the portfolio.
pub fn run_agent_bridge() -> i32 {
    exit_code_on_new_runtime(async {
        let connector: agent_connection::bridge::Connector = Box::new(|| {
            Box::pin(async {
                let data_dir = shared::infrastructure::app_directories::resolve_local_data_dir()?;
                agent_connection::channel::reach(&data_dir).await
            })
        });
        match agent_connection::bridge::serve(tokio::io::stdin(), tokio::io::stdout(), connector)
            .await
        {
            Ok(()) => 0,
            Err(_) => 1,
        }
    })
}

/// CLI-010 — whether the arguments start a command: `holding`, `account`, `asset`, or a
/// request for the command line's help. Anything else is the program's other starts.
pub fn starts_command_line(args: &[String]) -> bool {
    args.first().is_some_and(|first| {
        matches!(
            first.as_str(),
            "holding" | "account" | "asset" | "--help" | "-h"
        )
    })
}

/// Runs a command on a runtime of its own, prints its result and returns its exit code.
fn execute_command_line(program: &str, args: &[String]) -> i32 {
    exit_code_on_new_runtime(async {
        let printed = command_line::headless::run(program, args).await;
        if let Some(text) = &printed.stdout {
            println!("{text}");
        }
        if let Some(text) = &printed.stderr {
            eprintln!("{text}");
        }
        printed.exit_code
    })
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
    initialize_tracing_to(log_dir, true)
}

/// Initialize tracing: append to `app.log`, and to stderr when `echo_to_stderr`.
fn initialize_tracing_to(log_dir: &std::path::Path, echo_to_stderr: bool) -> anyhow::Result<()> {
    let log_file = log_dir.join("app.log");

    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .with_context(|| "Failed to open log file")?;

    tracing_subscriber::registry()
        .with(fmt::layer().with_ansi(false).with_writer(file))
        .with(echo_to_stderr.then(|| fmt::layer().with_writer(std::io::stderr)))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .try_init()
        .with_context(|| "Failed to install the tracing subscriber")?;

    tracing::trace!(target: BACKEND, "Logging initialized. Log file: {}", log_file.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The core sets up its own logging (TODO-047): the log file is created in the given folder,
    // and a folder that does not exist is an error, not a panic.
    // The headless entry returns the exit code its run produces (TODO-047, SPF-020).
    // CLI-022 — the command-line entry returns the exit code of what it ran.
    #[test]
    fn the_command_line_entry_returns_the_exit_code() {
        let args = |line: &str| {
            line.split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            execute_command_line("folioneer", &args("holding --help")),
            0
        );
        assert_eq!(execute_command_line("folioneer", &args("holding move")), 2);
    }

    // CLI-010 — `holding` and a request for help start the command line; nothing else does.
    #[test]
    fn cli_010_a_command_or_a_request_for_help_starts_the_command_line() {
        let starts = |line: &str| {
            let args: Vec<String> = line.split_whitespace().map(str::to_string).collect();
            starts_command_line(&args)
        };
        assert!(starts("holding buy --account PEA"));
        assert!(starts("account list"));
        assert!(starts("asset list --json"));
        assert!(starts("--help"));
        assert!(starts("-h"));
        assert!(!starts(""));
        assert!(!starts("--scheduled-fetch"));
        assert!(!starts("--scheduled-fetch --help"));
        assert!(!starts("portfolio.db"));
    }

    // AGT-012 — `--mcp` first starts the bridge and nothing else: not a command of the
    // command line, and not when it comes after another argument.
    #[test]
    fn agt_012_mcp_as_the_first_argument_starts_the_bridge_and_nothing_else() {
        let args =
            |line: &str| -> Vec<String> { line.split_whitespace().map(str::to_string).collect() };
        assert!(starts_agent_bridge(&args("--mcp")));
        assert!(starts_agent_bridge(&args("--mcp anything")));
        assert!(!starts_agent_bridge(&args("")));
        assert!(!starts_agent_bridge(&args("holding --mcp")));
        assert!(!starts_agent_bridge(&args("--scheduled-fetch --mcp")));
        assert!(!starts_command_line(&args("--mcp")));
    }

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
