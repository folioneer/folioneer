// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::AccountCreationUseCase;
use crate::context::account::{Account, AccountError};
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
    /// Whether the % management-fee mechanism is enabled (FEE-075). New accounts
    /// default to false; the creation form can opt in.
    pub management_fees_enabled: bool,
    /// ACC-041 — the account's own country (ISO 3166-1 alpha-2), when it differs from the
    /// portfolio's; none otherwise.
    pub country: Option<String>,
    /// ACC-043 — the identifier of its tax wrapper (ACC-040); none when unset.
    pub wrapper: Option<String>,
}

/// Adds a new account, eagerly seeding its Cash Asset + 0-balance Cash Holding
/// (ACC-025, CSH-010 / CSH-012).
#[tauri::command]
#[specta::specta]
pub async fn add_account(
    uc: State<'_, AccountCreationUseCase>,
    dto: CreateAccountDTO,
) -> Result<Account, AccountError> {
    uc.create_classified(
        dto.name,
        dto.bank_name,
        dto.currency,
        dto.management_fees_enabled,
        dto.country,
        dto.wrapper,
    )
    .await
}
