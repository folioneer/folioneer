//! Windows Task Scheduler adapter (SPF-017) — unit-verified generated
//! definitions only (no Windows machine to verify live registration, per the
//! plan's platform decision).

use super::DailyFetchScheduler;
use anyhow::Context;
use async_trait::async_trait;
use std::process::Command;

/// Scheduled Task name used for the `/TN` argument and the task-definition XML.
pub const TASK_NAME: &str = "FolioneerFetch";

/// Generates the `schtasks /Create` argument list for registering the daily
/// task. `trigger_time` is the local wall-clock time ("HH:MM", SPF-014);
/// `exe_path` is the current executable's absolute path, invoked with
/// `--scheduled-fetch` (SPF-020). `/F` forces overwrite on re-registration
/// (SPF-012 — changing the time re-registers).
pub fn create_task_args(trigger_time: &str, exe_path: &str) -> Vec<String> {
    vec![
        "/Create".to_string(),
        "/SC".to_string(),
        "DAILY".to_string(),
        "/TN".to_string(),
        TASK_NAME.to_string(),
        "/TR".to_string(),
        format!("{exe_path} --scheduled-fetch"),
        "/ST".to_string(),
        trigger_time.to_string(),
        "/F".to_string(),
    ]
}

/// Generates the Task Scheduler XML task definition. `StartWhenAvailable` is
/// `true` so a missed trigger (machine off/asleep) fires at the next
/// opportunity (SPF-022 catch-up), mirroring systemd's `Persistent=true`.
pub fn task_definition_xml(trigger_time: &str, exe_path: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>2026-01-01T{trigger_time}:00</StartBoundary>
      <ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>
    </CalendarTrigger>
  </Triggers>
  <Settings>
    <StartWhenAvailable>true</StartWhenAvailable>
  </Settings>
  <Actions>
    <Exec>
      <Command>{exe_path}</Command>
      <Arguments>--scheduled-fetch</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// Production [`DailyFetchScheduler`] backed by `schtasks.exe` (SPF-017).
pub struct WindowsTaskScheduler;

fn register_blocking(trigger_time: &str) -> anyhow::Result<()> {
    let executable_path =
        std::env::current_exe().context("failed to resolve the current executable path")?;
    let arguments = create_task_args(trigger_time, &executable_path.to_string_lossy());
    let status = Command::new("schtasks")
        .args(&arguments)
        .status()
        .context("failed to run `schtasks /Create`")?;
    anyhow::ensure!(status.success(), "`schtasks /Create` exited with {status}");
    Ok(())
}

fn remove_blocking() -> anyhow::Result<()> {
    // Must be a no-op when nothing is registered.
    if !is_registered_blocking()? {
        return Ok(());
    }
    let status = Command::new("schtasks")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .status()
        .context("failed to run `schtasks /Delete`")?;
    anyhow::ensure!(status.success(), "`schtasks /Delete` exited with {status}");
    Ok(())
}

fn is_registered_blocking() -> anyhow::Result<bool> {
    let output = Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .output()
        .context("failed to run `schtasks /Query`")?;
    Ok(output.status.success())
}

#[async_trait]
impl DailyFetchScheduler for WindowsTaskScheduler {
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

    // SPF-012/014 — the create args register a DAILY task at the local trigger time.
    #[test]
    fn create_task_args_registers_daily_at_trigger_time() {
        let args = create_task_args("22:15", r"C:\Program Files\Folioneer\folioneer.exe");
        assert!(args.contains(&"/SC".to_string()));
        assert!(args.contains(&"DAILY".to_string()));
        assert!(args.contains(&"/ST".to_string()));
        assert!(args.contains(&"22:15".to_string()));
        assert!(
            args.contains(&"/F".to_string()),
            "must force overwrite on re-registration"
        );
    }

    // SPF-020 — the task action invokes the executable with --scheduled-fetch.
    #[test]
    fn create_task_args_invokes_executable_with_scheduled_fetch_flag() {
        let args = create_task_args("22:15", r"C:\folioneer.exe");
        assert!(
            args.iter()
                .any(|a| a.contains(r"C:\folioneer.exe --scheduled-fetch")),
            "got: {args:?}"
        );
    }

    // SPF-022 — the task definition XML sets StartWhenAvailable so a missed
    // trigger catches up at the next opportunity.
    #[test]
    fn task_definition_xml_sets_start_when_available() {
        let xml = task_definition_xml("22:15", r"C:\folioneer.exe");
        assert!(
            xml.contains("<StartWhenAvailable>true</StartWhenAvailable>"),
            "got:\n{xml}"
        );
        assert!(
            xml.contains("T22:15:00"),
            "must encode the local trigger time, got:\n{xml}"
        );
        assert!(xml.contains("--scheduled-fetch"), "got:\n{xml}");
    }
}
