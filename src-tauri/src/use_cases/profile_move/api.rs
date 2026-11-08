// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;

use tauri::State;

use super::ProfileMoveUseCase;
use crate::context::account::{Account, AccountError};

/// PFL-031 — moves an account into another profile with its whole history.
#[tauri::command]
#[specta::specta]
pub async fn move_account_to_profile(
    uc: State<'_, Arc<ProfileMoveUseCase>>,
    account_id: String,
    profile_id: String,
) -> Result<Account, AccountError> {
    uc.move_account(&account_id, &profile_id).await
}
