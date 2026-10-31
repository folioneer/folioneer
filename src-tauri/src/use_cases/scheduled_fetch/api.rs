// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use crate::use_cases::price_provider_choice::ChosenPriceProvider;

use super::error::ScheduledFetchError;
use super::orchestrator::ScheduledFetchOrchestrator;
use super::repository::ScheduledFetchStatus;

/// Applies a configuration change to the daily price download (SPF-010–013,
/// SPF-019). Registers/removes the OS schedule before persisting so the
/// configuration never contradicts the OS schedule.
#[tauri::command]
#[specta::specta]
pub async fn configure_scheduled_fetch(
    uc: State<'_, Arc<ScheduledFetchOrchestrator>>,
    chosen: State<'_, Arc<ChosenPriceProvider>>,
    enabled: bool,
    trigger_time: String,
) -> Result<(), ScheduledFetchError> {
    if !chosen.is_on() {
        return Err(ScheduledFetchError::NoExternalProvider);
    }
    uc.configure(enabled, trigger_time).await
}

/// Reads the current configuration and the most recent run for the Settings
/// section's status line (SPF-052).
#[tauri::command]
#[specta::specta]
pub async fn get_scheduled_fetch_status(
    uc: State<'_, Arc<ScheduledFetchOrchestrator>>,
    chosen: State<'_, Arc<ChosenPriceProvider>>,
) -> Result<ScheduledFetchStatus, ScheduledFetchError> {
    if !chosen.is_on() {
        return Err(ScheduledFetchError::NoExternalProvider);
    }
    uc.status().await
}
