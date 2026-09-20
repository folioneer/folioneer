// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::{AccountDeletionSummary, AccountDeletionUseCase};
use crate::context::account::AccountError;
use tauri::State;

/// Returns the number of active holdings and transactions for an account (ACC-020).
///
/// Used by the frontend to decide whether to show the standard or reinforced
/// delete confirmation dialog (ACC-018 vs ACC-019).
#[tauri::command]
#[specta::specta]
pub async fn get_account_deletion_summary(
    uc: State<'_, AccountDeletionUseCase>,
    account_id: String,
) -> Result<AccountDeletionSummary, AccountError> {
    uc.get_summary(&account_id).await
}
