//! macOS launchd adapter (SPF-017) — unit-verified generated definitions only
//! (no macOS machine to verify live registration, per the plan's platform
//! decision).

use super::DailyFetchScheduler;
use anyhow::Context;
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Command;

/// launchd label used for the plist filename and `<Label>` entry.
pub const LABEL: &str = "com.folioneer.fetch";

/// Splits a well-formed "HH:MM" trigger time into its `(hour, minute)` integer
/// components for the `StartCalendarInterval` plist keys. Panics are
/// unreachable in production because `ScheduledFetchConfiguration::new`
/// already validated the format before a scheduler call is ever made (SPF-019).
fn split_trigger_time(trigger_time: &str) -> (u32, u32) {
    let mut parts = trigger_time.splitn(2, ':');
    let hour = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let minute = parts.next().unwrap_or("0").parse().unwrap_or(0);
    (hour, minute)
}

/// Generates the `com.folioneer.fetch.plist` content. `StartCalendarInterval`
/// with `Hour`/`Minute` fires once per calendar day at the local trigger time
/// (SPF-014); launchd itself catches up a missed run on wake (SPF-022 —
/// no `Persistent`-equivalent key needed, this is launchd's default behavior
/// for `StartCalendarInterval`). The program is invoked with `--scheduled-fetch`
/// (SPF-020).
pub fn plist_content(trigger_time: &str, exe_path: &str) -> String {
    let (hour, minute) = split_trigger_time(trigger_time);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe_path}</string>
        <string>--scheduled-fetch</string>
    </array>
    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key>
        <integer>{hour}</integer>
        <key>Minute</key>
        <integer>{minute}</integer>
    </dict>
</dict>
</plist>
"#
    )
}

/// Production [`DailyFetchScheduler`] backed by `launchctl` (SPF-017).
pub struct LaunchdScheduler;

/// LaunchAgents directory the plist is written to.
fn launch_agents_directory() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join("Library/LaunchAgents"))
}

fn plist_path() -> anyhow::Result<PathBuf> {
    Ok(launch_agents_directory()?.join(format!("{LABEL}.plist")))
}

/// `launchctl bootstrap`/`bootout` require the per-user GUI domain (`gui/<uid>`).
fn gui_domain() -> anyhow::Result<String> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .context("failed to run `id -u`")?;
    anyhow::ensure!(
        output.status.success(),
        "`id -u` exited with {}",
        output.status
    );
    Ok(format!(
        "gui/{}",
        String::from_utf8_lossy(&output.stdout).trim()
    ))
}

fn register_blocking(trigger_time: &str) -> anyhow::Result<()> {
    let executable_path =
        std::env::current_exe().context("failed to resolve the current executable path")?;
    let launch_agents_directory = launch_agents_directory()?;
    std::fs::create_dir_all(&launch_agents_directory)
        .with_context(|| format!("failed to create {}", launch_agents_directory.display()))?;
    let plist_path = plist_path()?;
    std::fs::write(
        &plist_path,
        plist_content(trigger_time, &executable_path.to_string_lossy()),
    )
    .context("failed to write the launchd plist")?;
    let gui_domain = gui_domain()?;
    // `bootstrap` rejects an already-loaded label: re-registration requires a
    // prior `bootout`, which fails harmlessly when the label is not loaded.
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{gui_domain}/{LABEL}")])
        .status();
    let status = Command::new("launchctl")
        .args(["bootstrap", &gui_domain])
        .arg(&plist_path)
        .status()
        .context("failed to run `launchctl bootstrap`")?;
    anyhow::ensure!(
        status.success(),
        "`launchctl bootstrap` exited with {status}"
    );
    Ok(())
}

fn remove_blocking() -> anyhow::Result<()> {
    // Must be a no-op when nothing is registered: a failing bootout is not an error.
    let gui_domain = gui_domain()?;
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{gui_domain}/{LABEL}")])
        .status();
    let plist_path = plist_path()?;
    if plist_path.exists() {
        std::fs::remove_file(&plist_path)
            .with_context(|| format!("failed to remove {}", plist_path.display()))?;
    }
    Ok(())
}

fn is_registered_blocking() -> anyhow::Result<bool> {
    let gui_domain = gui_domain()?;
    let output = Command::new("launchctl")
        .args(["print", &format!("{gui_domain}/{LABEL}")])
        .output()
        .context("failed to run `launchctl print`")?;
    Ok(output.status.success())
}

#[async_trait]
impl DailyFetchScheduler for LaunchdScheduler {
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

    // SPF-014 — the plist StartCalendarInterval hour/minute matches the trigger time.
    #[test]
    fn plist_content_encodes_trigger_time_hour_and_minute() {
        let content = plist_content(
            "22:15",
            "/Applications/Folioneer.app/Contents/MacOS/folioneer",
        );
        assert!(
            content.contains("<key>Hour</key>\n        <integer>22</integer>"),
            "got:\n{content}"
        );
        assert!(
            content.contains("<key>Minute</key>\n        <integer>15</integer>"),
            "got:\n{content}"
        );
    }

    // SPF-020 — the program arguments invoke the executable with --scheduled-fetch.
    #[test]
    fn plist_content_invokes_executable_with_scheduled_fetch_flag() {
        let content = plist_content(
            "06:05",
            "/Applications/Folioneer.app/Contents/MacOS/folioneer",
        );
        assert!(
            content
                .contains("<string>/Applications/Folioneer.app/Contents/MacOS/folioneer</string>"),
            "got:\n{content}"
        );
        assert!(
            content.contains("<string>--scheduled-fetch</string>"),
            "got:\n{content}"
        );
    }

    #[test]
    fn split_trigger_time_parses_hour_and_minute() {
        assert_eq!(split_trigger_time("22:15"), (22, 15));
        assert_eq!(split_trigger_time("06:05"), (6, 5));
        assert_eq!(split_trigger_time("00:00"), (0, 0));
    }
}
