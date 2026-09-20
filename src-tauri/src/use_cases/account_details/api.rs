// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::orchestrator::{AccountDetailsResponse, AccountDetailsUseCase};
use crate::context::account::AccountError;
use tauri::State;

/// Returns the full account details view for the given account (ACD-012 to ACD-041).
///
/// `as_of_date` selects the valuation date: `None` is the live view (today),
/// `Some("YYYY-MM-DD")` reconstructs the account read-only as it stood on a past date.
#[tauri::command]
#[specta::specta]
pub async fn get_account_details(
    state: State<'_, AccountDetailsUseCase>,
    account_id: String,
    as_of_date: Option<String>,
) -> Result<AccountDetailsResponse, AccountError> {
    state
        .get_account_details(&account_id, as_of_date.as_deref())
        .await
}
