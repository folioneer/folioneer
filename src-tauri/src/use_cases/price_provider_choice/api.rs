// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use super::error::PriceProviderChoiceError;
use super::orchestrator::{PriceProviderChoice, PriceProviderChoiceUseCase};

/// The External provider chosen on this computer, and what the build offers (MKT-214).
#[tauri::command]
#[specta::specta]
pub fn get_price_provider_choice(
    uc: State<'_, Arc<PriceProviderChoiceUseCase>>,
) -> PriceProviderChoice {
    uc.read()
}

/// Chooses an External provider on this computer, or none (MKT-214).
#[tauri::command]
#[specta::specta]
pub async fn set_price_provider_choice(
    uc: State<'_, Arc<PriceProviderChoiceUseCase>>,
    chosen: Option<String>,
) -> Result<PriceProviderChoice, PriceProviderChoiceError> {
    uc.set(chosen).await
}
