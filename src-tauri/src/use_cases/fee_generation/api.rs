// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::error::FeeGenerationError;
use super::orchestrator::FeeGenerationOrchestrator;
use tauri::State;

/// Applies all due management fee deductions across all active schedules (FEE-040).
///
/// Lazy catch-up: for each active schedule, generates one deduction per completed
/// period since `last_applied_period`, dated at the period boundary, in
/// chronological order. Skips periods where the holding quantity was 0 (FEE-047)
/// or where the deduction would oversell (FEE-044). Advances the cursor even
/// for skipped periods (FEE-043).
#[tauri::command]
#[specta::specta]
pub async fn apply_due_fee_deductions(
    uc: State<'_, FeeGenerationOrchestrator>,
) -> Result<(), FeeGenerationError> {
    uc.apply_due_fee_deductions().await
}
