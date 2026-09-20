//! Where a build keeps its data and its logs: the installed application's folders for a
//! release build, development folders of their own for a debug build.

use std::path::{Path, PathBuf};

/// The application identifier Tauri derives its per-app directories from
/// (`tauri.conf.json` → `identifier`). Code without a Tauri handle reproduces
/// `app_local_data_dir()` = platform data-local dir + this identifier; a mismatch would
/// silently split the application onto two databases (guarded by a test below).
const APP_IDENTIFIER: &str = "com.folioneer.desktop";

/// The identifier a debug build keeps its folders under, so a development run never
/// opens the installed application's data.
const DEVELOPMENT_IDENTIFIER: &str = "com.folioneer.desktop.dev";

/// The data folder of a build under the platform's data-local directory.
fn local_data_dir_in(data_local_base: &Path, debug_build: bool) -> PathBuf {
    data_local_base.join(folder_identifier(debug_build))
}

/// The log folder that sits next to a build's data (every platform but macOS, which
/// keeps logs under `~/Library/Logs`).
fn log_dir_in(local_data_dir: &Path) -> PathBuf {
    local_data_dir.join("logs")
}

/// This build's data folder, resolved without a Tauri handle.
pub fn resolve_local_data_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|base| local_data_dir_in(&base, cfg!(debug_assertions)))
}

/// This build's log folder, resolved without a Tauri handle — where Tauri's
/// `app_log_dir()` puts it for the same identifier.
pub fn resolve_log_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|home| {
            home.join("Library/Logs")
                .join(folder_identifier(cfg!(debug_assertions)))
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        resolve_local_data_dir().map(|data_dir| log_dir_in(&data_dir))
    }
}

/// The data and log folders of a debug build: the isolated data folder an E2E run
/// injects, the development folder otherwise; logs go to the development log folder
/// either way.
pub fn development_directories(e2e_data_dir: Option<PathBuf>) -> Option<(PathBuf, PathBuf)> {
    let local_data_dir = e2e_data_dir.or_else(resolve_local_data_dir)?;
    Some((local_data_dir, resolve_log_dir()?))
}

fn folder_identifier(debug_build: bool) -> &'static str {
    if debug_build {
        DEVELOPMENT_IDENTIFIER
    } else {
        APP_IDENTIFIER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_identifier_matches_tauri_conf() {
        let conf = include_str!("../../../tauri.conf.json");
        assert!(
            conf.contains(&format!("\"identifier\": \"{APP_IDENTIFIER}\"")),
            "APP_IDENTIFIER must match tauri.conf.json's identifier"
        );
    }

    // #040 — a release build keeps its data where Tauri puts the installed application's.
    #[test]
    fn a_release_build_resolves_the_installed_application_folder() {
        let base = Path::new("/home/someone/.local/share");
        assert_eq!(
            local_data_dir_in(base, false),
            base.join("com.folioneer.desktop")
        );
    }

    // #040 — a debug build resolves a folder of its own, neither the installed
    // application's nor nested inside it, so nothing it writes (database, logs) lands there.
    #[test]
    fn a_debug_build_resolves_a_folder_outside_the_installed_application_folder() {
        let base = Path::new("/home/someone/.local/share");
        let installed = local_data_dir_in(base, false);
        let development = local_data_dir_in(base, true);

        assert_eq!(development, base.join("com.folioneer.desktop.dev"));
        assert!(!development.starts_with(&installed));
        assert!(!log_dir_in(&development).starts_with(&installed));
    }

    // #040 — tests run as a debug build: what such a build resolves for itself, data and
    // logs, stays out of the installed application's folder.
    #[test]
    fn a_development_run_keeps_data_and_logs_out_of_the_installed_application_folder() {
        let base = dirs::data_local_dir().expect("platform data folder");
        let installed = local_data_dir_in(&base, false);

        let (local_data_dir, log_dir) = development_directories(None).expect("directories");

        assert_eq!(local_data_dir, base.join(DEVELOPMENT_IDENTIFIER));
        assert!(!log_dir.starts_with(&installed));
    }

    // #040 — an E2E run keeps its injected data folder, and logs beside the development
    // data rather than beside the installed application's.
    #[test]
    fn an_e2e_run_keeps_its_own_data_folder_and_logs_with_the_development_run() {
        let base = dirs::data_local_dir().expect("platform data folder");
        let e2e_data_dir = PathBuf::from("/tmp/e2e-run");

        let (local_data_dir, log_dir) =
            development_directories(Some(e2e_data_dir.clone())).expect("directories");

        assert_eq!(local_data_dir, e2e_data_dir);
        assert!(!log_dir.starts_with(local_data_dir_in(&base, false)));
    }
}
