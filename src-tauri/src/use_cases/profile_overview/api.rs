// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;

use tauri::State;

use super::{ProfileOverview, ProfileOverviewUseCase};
use crate::context::account::AccountError;

/// PFL-055 — every profile with its own figures, never added across profiles.
#[tauri::command]
#[specta::specta]
pub async fn get_profile_overview(
    uc: State<'_, Arc<ProfileOverviewUseCase>>,
) -> Result<ProfileOverview, AccountError> {
    uc.read().await
}
