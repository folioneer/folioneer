// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::orchestrator::{AccountSummaries, AccountSummaryUseCase};
use crate::context::account::AccountError;
use tauri::State;

/// Returns one `AccountSummary` per non-deleted account and their portfolio total
/// in the reference currency (ACC-021, ACC-027).
#[tauri::command]
#[specta::specta]
pub async fn get_account_summaries(
    state: State<'_, AccountSummaryUseCase>,
) -> Result<AccountSummaries, AccountError> {
    state.get_account_summaries().await
}
