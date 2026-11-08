//! PFL-031 — an account moved into another profile: the profile's existence checked against
//! the profiles, the move made by the accounts.

/// Tauri command handler for the move.
#[cfg(feature = "app")]
mod api;
/// The orchestrator: checks the profile, then has the account service move the account.
mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use orchestrator::ProfileMoveUseCase;
