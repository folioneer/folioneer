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
    holder: Option<String>,
) -> Result<AccountSummaries, AccountError> {
    // ACC-021 / ACC-051 — the rows of one holder when one is given; the total stays every
    // account's of the profile in use.
    state.get_account_summaries_for_holder(holder).await
}
