//! Which External provider this computer fetches prices from, if any (MKT-214, ADR-024). The
//! build offers a list (`extensions.rs`); the choice is a file of this computer's data folder,
//! so it is never synced and the headless run reads it before opening anything (SPF-072).

/// Tauri commands (`get_price_provider_choice`, `set_price_provider_choice`).
#[cfg(feature = "app")]
pub mod api;
/// Flat wire-facing error enum.
pub mod error;
/// Reads and changes the choice.
pub mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use error::PriceProviderChoiceError;
pub use orchestrator::{FetchScheduleFollower, PriceProviderChoice, PriceProviderChoiceUseCase};
