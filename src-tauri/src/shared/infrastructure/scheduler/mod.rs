//! Daily fetch scheduler abstraction (SPF-012, SPF-017) — registers, removes, and
//! probes the OS-native daily task that triggers the scheduled price download.
//!
//! Three platform adapters ship: [`systemd`] (Linux, fully verified), [`windows_task`]
//! (Windows, unit-verified generated definitions only), and [`launchd`] (macOS,
//! unit-verified generated definitions only) — SPF-017. [`NoopScheduler`] is what a
//! debug build gets, so a development or E2E run never touches the host's real
//! task scheduler — the installed application owns it.

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

/// Inert scheduler of a debug build: a development run leaves the installed
/// application's daily schedule alone, and E2E specs exercise the full
/// FE ↔ BE ↔ SQLite stack without touching the CI host's real task scheduler.
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

/// Whether a build gets the [`NoopScheduler`]: only a release build may register,
/// remove or probe the host's daily schedule.
fn uses_inert_scheduler(debug_build: bool) -> bool {
    debug_build
}

/// Returns the scheduler adapter for the current platform (SPF-017); a debug build
/// gets the [`NoopScheduler`].
pub fn platform_scheduler() -> std::sync::Arc<dyn DailyFetchScheduler> {
    if uses_inert_scheduler(cfg!(debug_assertions)) {
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

    // #040 — only a release build owns the host's daily schedule: a development or E2E
    // run must neither repoint it at a debug binary nor remove it.
    #[test]
    fn only_a_release_build_touches_the_host_schedule() {
        assert!(!uses_inert_scheduler(false));
        assert!(uses_inert_scheduler(true));
    }

    // #040 — tests run as a debug build, so the scheduler they get reports nothing
    // registered even on a computer whose installed application has a daily schedule.
    #[tokio::test]
    async fn a_debug_build_gets_the_inert_scheduler() {
        let scheduler = platform_scheduler();
        assert!(!scheduler.is_registered().await.unwrap());
    }
}
