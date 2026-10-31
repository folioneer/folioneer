//! The isolated data folder an E2E run injects, honoured by debug builds only.

use std::path::PathBuf;

/// Environment variable through which an E2E run names its data folder.
pub const E2E_DATA_DIR_VARIABLE: &str = "FOLIONEER_E2E_DATA_DIR";

/// Environment variable through which an E2E run names the file the system dialog would give.
pub const E2E_PICKED_FILE_VARIABLE: &str = "FOLIONEER_E2E_PICKED_FILE";

/// The file an E2E run picks without the system dialog, which no test can drive; `None`
/// outside one. Release builds never honour it.
pub fn e2e_picked_file() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        std::env::var_os(E2E_PICKED_FILE_VARIABLE).map(PathBuf::from)
    } else {
        None
    }
}

/// The data folder of an E2E run, or `None` outside one. Release builds never honour it.
pub fn e2e_data_dir() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        std::env::var_os(E2E_DATA_DIR_VARIABLE).map(PathBuf::from)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO-033 — an E2E run names its data folder through the variable. The only test that
    // sets the variable.
    #[test]
    fn an_e2e_run_is_recognised_by_its_variable() {
        std::env::remove_var(E2E_DATA_DIR_VARIABLE);
        assert_eq!(e2e_data_dir(), None);

        std::env::set_var(E2E_DATA_DIR_VARIABLE, "/tmp/e2e-run");
        let recognised = e2e_data_dir();
        std::env::remove_var(E2E_DATA_DIR_VARIABLE);

        assert_eq!(recognised, Some(PathBuf::from("/tmp/e2e-run")));
    }

    // IMP-060 — an E2E run names the file to pick; the only test that sets the variable.
    #[test]
    fn an_e2e_run_names_the_file_it_picks() {
        std::env::remove_var(E2E_PICKED_FILE_VARIABLE);
        assert_eq!(e2e_picked_file(), None);

        std::env::set_var(E2E_PICKED_FILE_VARIABLE, "/tmp/broker.csv");
        let picked = e2e_picked_file();
        std::env::remove_var(E2E_PICKED_FILE_VARIABLE);

        assert_eq!(picked, Some(PathBuf::from("/tmp/broker.csv")));
    }
}
