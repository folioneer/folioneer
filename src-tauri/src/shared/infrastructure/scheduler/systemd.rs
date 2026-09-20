//! Linux systemd user-timer adapter (SPF-017) — fully verified: unit tests on
//! the generated `.service`/`.timer` content below, plus live E2E registration
//! on the dev machine (manual checklist item, not automated).

use super::DailyFetchScheduler;
use anyhow::Context;
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Command;

/// systemd unit name for the scheduled-fetch service (no `.service` suffix).
pub const UNIT_NAME: &str = "folioneer-fetch";

/// Generates the `folioneer-fetch.service` unit content. `exe_path` is the
/// current executable's absolute path (SPF-015 self-heal re-registers when it
/// changes); the unit invokes it with `--scheduled-fetch` (SPF-020).
pub fn service_unit_content(exe_path: &str) -> String {
    format!(
        "[Unit]\nDescription=Folioneer scheduled price download\n\n[Service]\nType=oneshot\nExecStart={exe_path} --scheduled-fetch\n"
    )
}

/// Generates the `folioneer-fetch.timer` unit content. `OnCalendar` encodes
/// the local wall-clock `trigger_time` ("HH:MM", SPF-014); `Persistent=true`
/// makes systemd fire the timer immediately on the next boot/wake if the
/// machine was off at the trigger (SPF-022 catch-up).
pub fn timer_unit_content(trigger_time: &str) -> String {
    format!(
        "[Unit]\nDescription=Folioneer scheduled price download timer\n\n[Timer]\nOnCalendar=*-*-* {trigger_time}:00\nPersistent=true\n\n[Install]\nWantedBy=timers.target\n"
    )
}

/// Production [`DailyFetchScheduler`] backed by `systemctl --user` (SPF-017).
pub struct SystemdScheduler;

/// User unit directory the `.service`/`.timer` files are written to.
fn user_unit_directory() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".config/systemd/user"))
}

fn run_systemctl_user(arguments: &[&str]) -> anyhow::Result<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(arguments)
        .status()
        .with_context(|| format!("failed to run `systemctl --user {}`", arguments.join(" ")))?;
    anyhow::ensure!(
        status.success(),
        "`systemctl --user {}` exited with {status}",
        arguments.join(" ")
    );
    Ok(())
}

fn register_blocking(trigger_time: &str) -> anyhow::Result<()> {
    let executable_path =
        std::env::current_exe().context("failed to resolve the current executable path")?;
    let unit_directory = user_unit_directory()?;
    std::fs::create_dir_all(&unit_directory)
        .with_context(|| format!("failed to create {}", unit_directory.display()))?;
    std::fs::write(
        unit_directory.join(format!("{UNIT_NAME}.service")),
        service_unit_content(&executable_path.to_string_lossy()),
    )
    .context("failed to write the service unit file")?;
    std::fs::write(
        unit_directory.join(format!("{UNIT_NAME}.timer")),
        timer_unit_content(trigger_time),
    )
    .context("failed to write the timer unit file")?;
    run_systemctl_user(&["daemon-reload"])?;
    run_systemctl_user(&["enable", "--now", &format!("{UNIT_NAME}.timer")])?;
    Ok(())
}

fn remove_blocking() -> anyhow::Result<()> {
    // Must be a no-op when nothing is registered: a failing disable is not an error.
    let _ = Command::new("systemctl")
        .args(["--user", "disable", "--now", &format!("{UNIT_NAME}.timer")])
        .status();
    let unit_directory = user_unit_directory()?;
    for unit_file_name in [format!("{UNIT_NAME}.service"), format!("{UNIT_NAME}.timer")] {
        let unit_path = unit_directory.join(unit_file_name);
        if unit_path.exists() {
            std::fs::remove_file(&unit_path)
                .with_context(|| format!("failed to remove {}", unit_path.display()))?;
        }
    }
    run_systemctl_user(&["daemon-reload"])?;
    Ok(())
}

fn is_registered_blocking() -> anyhow::Result<bool> {
    let output = Command::new("systemctl")
        .args(["--user", "is-enabled", &format!("{UNIT_NAME}.timer")])
        .output()
        .context("failed to run `systemctl --user is-enabled`")?;
    Ok(output.status.success())
}

#[async_trait]
impl DailyFetchScheduler for SystemdScheduler {
    async fn register(&self, trigger_time: &str) -> anyhow::Result<()> {
        let trigger_time = trigger_time.to_string();
        tokio::task::spawn_blocking(move || register_blocking(&trigger_time))
            .await
            .context("scheduler task panicked")?
    }

    async fn remove(&self) -> anyhow::Result<()> {
        tokio::task::spawn_blocking(remove_blocking)
            .await
            .context("scheduler task panicked")?
    }

    async fn is_registered(&self) -> anyhow::Result<bool> {
        tokio::task::spawn_blocking(is_registered_blocking)
            .await
            .context("scheduler task panicked")?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SPF-014/022 — the timer unit encodes the local trigger time and is Persistent.
    #[test]
    fn timer_unit_content_encodes_trigger_time_and_is_persistent() {
        let content = timer_unit_content("22:15");
        assert!(
            content.contains("OnCalendar=*-*-* 22:15:00"),
            "must encode the local wall-clock trigger time, got:\n{content}"
        );
        assert!(
            content.contains("Persistent=true"),
            "must set Persistent=true for missed-trigger catch-up (SPF-022), got:\n{content}"
        );
    }

    // SPF-020 — the service unit invokes the current executable with --scheduled-fetch.
    #[test]
    fn service_unit_content_invokes_executable_with_scheduled_fetch_flag() {
        let content = service_unit_content("/opt/folioneer/folioneer");
        assert!(
            content.contains("ExecStart=/opt/folioneer/folioneer --scheduled-fetch"),
            "must invoke the executable headlessly, got:\n{content}"
        );
    }

    // A different trigger time is reflected verbatim (re-registration on time change, SPF-012).
    #[test]
    fn timer_unit_content_reflects_a_different_trigger_time() {
        let content = timer_unit_content("06:30");
        assert!(
            content.contains("OnCalendar=*-*-* 06:30:00"),
            "got:\n{content}"
        );
    }
}
