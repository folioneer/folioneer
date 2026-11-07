//! Windows Task Scheduler adapter (SPF-017) — registers the task from its XML
//! definition, so the definition the tests read is the one the Task Scheduler holds; a
//! Windows-only test registers one and reads it back.

use super::DailyFetchScheduler;
use anyhow::Context;
use async_trait::async_trait;
use std::process::Command;

/// Scheduled Task name used for the `/TN` argument.
pub const TASK_NAME: &str = "FolioneerFetch";

/// The `schtasks /Create` arguments that register `task_name` from the definition file at
/// `xml_path`. `/F` overwrites the task before it (SPF-012 — changing the time
/// re-registers).
pub fn create_task_args(task_name: &str, xml_path: &str) -> Vec<String> {
    ["/Create", "/XML", xml_path, "/TN", task_name, "/F"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

/// The Task Scheduler definition of the daily fetch: `exe_path --scheduled-fetch` every
/// day at the local `trigger_time` ("HH:MM", SPF-014, SPF-020). `StartWhenAvailable`
/// runs a missed trigger at the next opportunity (SPF-022), as systemd's
/// `Persistent=true` does, and a machine on battery neither skips nor stops the run (SPF-025).
/// The command and its argument are separate elements, so a path with a space is one
/// path; it is escaped for the XML.
pub fn task_definition_xml(trigger_time: &str, exe_path: &str) -> String {
    let command = escape_xml(exe_path);
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
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
  </Settings>
  <Actions>
    <Exec>
      <Command>{command}</Command>
      <Arguments>--scheduled-fetch</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// The five characters XML reads as markup, written as text.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// The definition as the file `schtasks /XML` reads: UTF-16 little-endian, as its
/// declaration says, behind the byte-order mark.
pub fn task_definition_file(xml: &str) -> Vec<u8> {
    [0xFF, 0xFE]
        .into_iter()
        .chain(xml.encode_utf16().flat_map(u16::to_le_bytes))
        .collect()
}

/// Production [`DailyFetchScheduler`] backed by `schtasks.exe` (SPF-017).
pub struct WindowsTaskScheduler;

/// Registers `task_name` to run `exe_path` daily at `trigger_time`, from its definition
/// written to a file of the temporary folder, removed once registered.
fn register_blocking(task_name: &str, trigger_time: &str, exe_path: &str) -> anyhow::Result<()> {
    let xml_path = std::env::temp_dir().join(format!("{task_name}-{}.xml", std::process::id()));
    std::fs::write(
        &xml_path,
        task_definition_file(&task_definition_xml(trigger_time, exe_path)),
    )
    .context("failed to write the task definition")?;
    let status = Command::new("schtasks")
        .args(create_task_args(task_name, &xml_path.to_string_lossy()))
        .status();
    let _ = std::fs::remove_file(&xml_path);
    let status = status.context("failed to run `schtasks /Create`")?;
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
        tokio::task::spawn_blocking(move || {
            let executable_path =
                std::env::current_exe().context("failed to resolve the current executable path")?;
            register_blocking(TASK_NAME, &trigger_time, &executable_path.to_string_lossy())
        })
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

    // SPF-012 / SPF-017 — the task is registered from its XML definition, under its name,
    // overwriting the one before (changing the time re-registers).
    #[test]
    fn the_task_is_registered_from_its_definition_file() {
        assert_eq!(
            create_task_args("FolioneerFetch", r"C:\Temp\task.xml"),
            [
                "/Create",
                "/XML",
                r"C:\Temp\task.xml",
                "/TN",
                "FolioneerFetch",
                "/F"
            ]
        );
    }

    // SPF-012/014/020/022 — the definition runs the executable with --scheduled-fetch daily
    // at the local trigger time, catches up a missed trigger, and does not skip or stop a
    // run on battery (SPF-025).
    #[test]
    fn the_definition_runs_daily_catches_up_and_ignores_the_battery() {
        let xml = task_definition_xml("22:15", r"C:\Program Files\Folioneer\folioneer.exe");
        for expected in [
            "T22:15:00</StartBoundary>",
            "<DaysInterval>1</DaysInterval>",
            "<StartWhenAvailable>true</StartWhenAvailable>",
            "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
            "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
            r"<Command>C:\Program Files\Folioneer\folioneer.exe</Command>",
            "<Arguments>--scheduled-fetch</Arguments>",
        ] {
            assert!(xml.contains(expected), "{expected} missing from:\n{xml}");
        }
    }

    // SPF-017 — a path the XML would misread is escaped, so the task runs that very file.
    #[test]
    fn the_definition_escapes_the_executable_path() {
        let xml = task_definition_xml("22:15", r"C:\Users\A&B <x>\folioneer.exe");
        assert!(
            xml.contains(r"<Command>C:\Users\A&amp;B &lt;x&gt;\folioneer.exe</Command>"),
            "got:\n{xml}"
        );
    }

    // The file schtasks reads is the definition in UTF-16 little-endian, as its
    // declaration says, behind its byte-order mark.
    #[test]
    fn the_definition_file_is_utf16_with_its_byte_order_mark() {
        let bytes = task_definition_file("<Task/>");
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect();
        assert_eq!(String::from_utf16(&units).expect("utf-16"), "<Task/>");
    }

    // SPF-017 / SPF-022 / SPF-025 — on Windows, the task registered is read back from the Task
    // Scheduler: it catches up a missed trigger, runs on battery, and runs the path given,
    // spaces included.
    #[cfg(windows)]
    #[test]
    fn the_registered_task_catches_up_and_runs_the_path_given() {
        /// Deletes the test task however the test ends.
        struct Registered(String);
        impl Drop for Registered {
            fn drop(&mut self) {
                let _ = Command::new("schtasks")
                    .args(["/Delete", "/TN", &self.0, "/F"])
                    .status();
            }
        }
        let task_name = format!("FolioneerFetchTest{}", std::process::id());
        let exe_path = r"C:\Program Files\Folioneer Test\folioneer.exe";
        register_blocking(&task_name, "22:15", exe_path).expect("registered");
        let registered = Registered(task_name);
        let output = Command::new("schtasks")
            .args(["/Query", "/TN", &registered.0, "/XML"])
            .output()
            .expect("queried");
        assert!(
            output.status.success(),
            "`schtasks /Query` exited with {}",
            output.status
        );
        let read: String = String::from_utf8_lossy(&output.stdout)
            .chars()
            .filter(|c| *c != '\0')
            .collect();
        for expected in [
            "<StartWhenAvailable>true</StartWhenAvailable>",
            "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
            "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
            &format!("<Command>{exe_path}</Command>"),
            "<Arguments>--scheduled-fetch</Arguments>",
        ] {
            assert!(read.contains(expected), "{expected} missing from:\n{read}");
        }
    }
}
