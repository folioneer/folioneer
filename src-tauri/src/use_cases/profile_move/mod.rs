//! PFL-031 — an account moved into another profile: the profile's existence checked against
//! the profiles, the move made by the accounts.

mod api;
mod orchestrator;

pub use api::*;
pub use orchestrator::ProfileMoveUseCase;
