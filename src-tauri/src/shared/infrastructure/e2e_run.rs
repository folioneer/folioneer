//! The isolated data folder an E2E run injects, honoured by debug builds only.

use std::path::PathBuf;

/// Environment variable through which an E2E run names its data folder.
pub const E2E_DATA_DIR_VARIABLE: &str = "FOLIONEER_E2E_DATA_DIR";

/// The data folder of an E2E run, or `None` outside one. Release builds never honour it.
pub fn e2e_data_dir() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        std::env::var_os(E2E_DATA_DIR_VARIABLE).map(PathBuf::from)
    } else {
        None
    }
}
