//! Daily fetch scheduler abstraction (SPF-012, SPF-017) — registers, removes, and
//! probes the OS-native daily task that triggers the scheduled price download.
//!
//! Three platform adapters ship: [`systemd`] (Linux, fully verified), [`windows_task`]
//! (Windows, unit-verified generated definitions only), and [`launchd`] (macOS,
//! unit-verified generated definitions only) — SPF-017. [`NoopScheduler`] is used
//! in E2E runs (`FOLIONEER_E2E_DATA_DIR`, debug builds only) so specs never
//! touch the host's real task scheduler.

/// macOS launchd adapter — generates the `.plist` definition (SPF-017).
pub mod launchd;
/// Linux systemd user-timer adapter — generates the `.service`/`.timer` units (SPF-017).
pub mod systemd;
/// Windows Task Scheduler adapter — generates `schtasks` args + task XML (SPF-017).
pub mod windows_task;

use async_trait::async_trait;

/// Registers, removes, and probes the OS-native daily scheduling facility used
/// to trigger the scheduled price download (SPF-012). Each platform adapter
/// registers the current executable path with the `--scheduled-fetch` argument.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait DailyFetchScheduler: Send + Sync {
    /// Registers (or re-registers, e.g. after a trigger-time change) the daily
    /// schedule at the given local wall-clock `trigger_time` ("HH:MM"). SPF-012.
    async fn register(&self, trigger_time: &str) -> anyhow::Result<()>;
    /// Removes the daily schedule. A no-op when nothing is registered. SPF-012.
    async fn remove(&self) -> anyhow::Result<()>;
    /// Returns whether the daily schedule is currently registered with the OS
    /// (used by the self-heal check, SPF-015).
    async fn is_registered(&self) -> anyhow::Result<bool>;
}

/// Inert scheduler used for E2E runs (`FOLIONEER_E2E_DATA_DIR`, debug builds
/// only) so specs exercise the full FE ↔ BE ↔ SQLite stack without touching the
/// CI host's real task scheduler.
#[derive(Debug, Default)]
pub struct NoopScheduler;

#[async_trait]
impl DailyFetchScheduler for NoopScheduler {
    async fn register(&self, _trigger_time: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn remove(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn is_registered(&self) -> anyhow::Result<bool> {
        Ok(false)
    }
}

/// Returns the scheduler adapter for the current platform (SPF-017).
/// E2E runs (`FOLIONEER_E2E_DATA_DIR`, debug builds only) get the
/// [`NoopScheduler`] so specs never touch the host's real task scheduler.
pub fn platform_scheduler() -> std::sync::Arc<dyn DailyFetchScheduler> {
    if super::e2e_run::e2e_data_dir().is_some() {
        return std::sync::Arc::new(NoopScheduler);
    }
    #[cfg(target_os = "linux")]
    {
        std::sync::Arc::new(systemd::SystemdScheduler)
    }
    #[cfg(target_os = "macos")]
    {
        std::sync::Arc::new(launchd::LaunchdScheduler)
    }
    #[cfg(target_os = "windows")]
    {
        std::sync::Arc::new(windows_task::WindowsTaskScheduler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The Noop scheduler never errors and never reports a registration.
    #[tokio::test]
    async fn noop_scheduler_register_and_remove_always_succeed() {
        let scheduler = NoopScheduler;
        assert!(scheduler.register("22:15").await.is_ok());
        assert!(scheduler.remove().await.is_ok());
        assert!(!scheduler.is_registered().await.unwrap());
    }

    // #033 — an E2E run names its data folder through the variable, and then gets the
    // scheduler that never touches the host. The only test that sets the variable.
    #[tokio::test]
    async fn an_e2e_run_is_recognised_and_gets_the_inert_scheduler() {
        use super::super::e2e_run::{e2e_data_dir, E2E_DATA_DIR_VARIABLE};

        std::env::remove_var(E2E_DATA_DIR_VARIABLE);
        assert_eq!(e2e_data_dir(), None);

        std::env::set_var(E2E_DATA_DIR_VARIABLE, "/tmp/e2e-run");
        assert_eq!(
            e2e_data_dir(),
            Some(std::path::PathBuf::from("/tmp/e2e-run"))
        );
        let scheduler = platform_scheduler();
        std::env::remove_var(E2E_DATA_DIR_VARIABLE);

        assert!(!scheduler.is_registered().await.unwrap());
    }
}
