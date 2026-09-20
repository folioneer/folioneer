// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::AccountCreationUseCase;
use crate::context::account::{Account, AccountError, UpdateFrequency};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

/// Parameters for creating a new account.
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct CreateAccountDTO {
    /// Display name.
    pub name: String,
    /// Bank brand name (free text, ACC-026); empty string means unset.
    pub bank_name: String,
    /// ISO 4217 currency code.
    pub currency: String,
    /// Update frequency.
    pub update_frequency: UpdateFrequency,
    /// Whether the % management-fee mechanism is enabled (FEE-075). New accounts
    /// default to false; the creation form can opt in.
    pub management_fees_enabled: bool,
}

/// Adds a new account, eagerly seeding its Cash Asset + 0-balance Cash Holding
/// (ACC-025, CSH-010 / CSH-012).
#[tauri::command]
#[specta::specta]
pub async fn add_account(
    uc: State<'_, AccountCreationUseCase>,
    dto: CreateAccountDTO,
) -> Result<Account, AccountError> {
    uc.create(
        dto.name,
        dto.bank_name,
        dto.currency,
        dto.update_frequency,
        dto.management_fees_enabled,
    )
    .await
}
