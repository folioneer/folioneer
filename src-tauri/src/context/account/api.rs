// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::domain::{
    Account, FeeFrequency, FeeSchedule, HoldingNote, ThresholdDirection, UpdateFrequency,
};
use crate::context::account::{AccountError, HoldingSnapshot, Transaction};
use crate::AppState;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

// --- DTOs ---

/// Parameters for updating an existing account.
#[derive(Debug, Serialize, Deserialize, Type)]
pub struct UpdateAccountDTO {
    /// Target account ID.
    pub id: String,
    /// New display name.
    pub name: String,
    /// Bank brand name (free text, ACC-026); empty string means unset.
    pub bank_name: String,
    /// ISO 4217 currency code.
    pub currency: String,
    /// New update frequency.
    pub update_frequency: UpdateFrequency,
    /// Whether the % management-fee mechanism is enabled (FEE-075).
    pub management_fees_enabled: bool,
}

// --- Commands ---

/// Retrieves all accounts.
#[tauri::command]
#[specta::specta]
pub async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<Account>, AccountError> {
    state.account_service.get_all().await
}

/// Updates an existing account.
#[tauri::command]
#[specta::specta]
pub async fn update_account(
    state: State<'_, AppState>,
    dto: UpdateAccountDTO,
) -> Result<Account, AccountError> {
    state
        .account_service
        .update(
            dto.id,
            dto.name,
            dto.bank_name,
            dto.currency,
            dto.update_frequency,
            dto.management_fees_enabled,
        )
        .await
}

/// Deletes an account (R5 — cascades to its holdings at the repo level).
#[tauri::command]
#[specta::specta]
pub async fn delete_account(state: State<'_, AppState>, id: String) -> Result<(), AccountError> {
    state.account_service.delete(&id).await
}

/// Returns the distinct asset IDs that have transactions for the given account (TXL-013).
#[tauri::command]
#[specta::specta]
pub async fn get_asset_ids_for_account(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<String>, AccountError> {
    state
        .account_service
        .get_asset_ids_for_account(&account_id)
        .await
}

/// Retrieves all transactions for an account/asset pair (TRX-036).
#[tauri::command]
#[specta::specta]
pub async fn get_transactions(
    state: State<'_, AppState>,
    account_id: String,
    asset_id: String,
) -> Result<Vec<Transaction>, AccountError> {
    state
        .account_service
        .get_transactions(&account_id, &asset_id)
        .await
}

/// TDI-010 — Returns the (account, asset) holding's quantity and VWAP average
/// cost as of `date`, for the trade-dialog insights.
#[tauri::command]
#[specta::specta]
pub async fn get_holding_snapshot_as_of(
    state: State<'_, AppState>,
    account_id: String,
    asset_id: String,
    date: String,
) -> Result<HoldingSnapshot, AccountError> {
    state
        .account_service
        .holding_snapshot_as_of(&account_id, &asset_id, &date)
        .await
}

/// Retrieves every transaction for an account across all assets, ordered
/// chronologically by `(date, created_at)` (TRX-036).
#[tauri::command]
#[specta::specta]
pub async fn get_all_transactions_for_account(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<Transaction>, AccountError> {
    state
        .account_service
        .get_all_transactions_for_account(&account_id)
        .await
}

// =============================================================================
// Management Fee Schedule — DTOs + commands (FEE-030/060/062)
// =============================================================================

/// Parameters for creating a recurring fee schedule (FEE-030).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CreateFeeScheduleDTO {
    /// Account the schedule applies to.
    pub account_id: String,
    /// The charged asset.
    pub asset_id: String,
    /// Annual fee rate in micro-percent (1% = 1_000_000), strictly positive and < 100% (FEE-032).
    pub annual_rate_percent_micros: i64,
    /// Deduction cadence (FEE-034).
    pub frequency: FeeFrequency,
    /// First business date deductions are generated from (YYYY-MM-DD, FEE-032).
    pub start_date: String,
    /// Optional date after which no further deductions are generated (FEE-045).
    pub end_date: Option<String>,
}

/// Parameters for editing a fee schedule (FEE-060/061). `frequency` and
/// `start_date` are intentionally absent — they are immutable after creation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct UpdateFeeScheduleDTO {
    /// Account the schedule applies to.
    pub account_id: String,
    /// The charged asset (identifies the schedule together with `account_id`).
    pub asset_id: String,
    /// New annual fee rate in micro-percent (FEE-032).
    pub annual_rate_percent_micros: i64,
    /// New optional end date (FEE-045).
    pub end_date: Option<String>,
    /// Whether the schedule is active; `false` pauses generation (FEE-061).
    pub active: bool,
}

/// Creates a recurring management fee schedule for an (account, asset) pair (FEE-030).
#[tauri::command]
#[specta::specta]
pub async fn create_fee_schedule(
    state: State<'_, AppState>,
    dto: CreateFeeScheduleDTO,
) -> Result<FeeSchedule, AccountError> {
    state
        .account_service
        .create_fee_schedule(
            &dto.account_id,
            dto.asset_id,
            dto.annual_rate_percent_micros,
            dto.frequency,
            dto.start_date,
            dto.end_date,
        )
        .await
}

/// Edits an existing fee schedule's rate, end date, and active flag (FEE-060/061).
#[tauri::command]
#[specta::specta]
pub async fn update_fee_schedule(
    state: State<'_, AppState>,
    dto: UpdateFeeScheduleDTO,
) -> Result<FeeSchedule, AccountError> {
    state
        .account_service
        .update_fee_schedule(
            &dto.account_id,
            &dto.asset_id,
            dto.annual_rate_percent_micros,
            dto.end_date,
            dto.active,
        )
        .await
}

/// Deletes the fee schedule for an (account, asset) pair (FEE-062, silent if absent).
#[tauri::command]
#[specta::specta]
pub async fn delete_fee_schedule(
    state: State<'_, AppState>,
    account_id: String,
    asset_id: String,
) -> Result<(), AccountError> {
    state
        .account_service
        .delete_fee_schedule(&account_id, &asset_id)
        .await
}

/// Returns the fee schedule for an (account, asset) pair, or `None` (FEE-030).
#[tauri::command]
#[specta::specta]
pub async fn get_fee_schedule(
    state: State<'_, AppState>,
    account_id: String,
    asset_id: String,
) -> Result<Option<FeeSchedule>, AccountError> {
    state
        .account_service
        .get_fee_schedule(&account_id, &asset_id)
        .await
}

// =============================================================================
// Holding Note — DTOs + commands (HNO-020/021)
// =============================================================================

/// Parameters for creating or replacing a holding note (HNO-020).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct UpsertHoldingNoteDTO {
    /// Account the note belongs to.
    pub account_id: String,
    /// The noted asset (identifies the note together with `account_id`).
    pub asset_id: String,
    /// Note text, 1-500 chars after trimming (HNO-011).
    pub text: String,
    /// Optional alarm threshold as a nominal asset-currency share price in micros (HNO-031).
    pub threshold_price: Option<i64>,
    /// Optional alarm direction; both alarm fields or neither (HNO-011).
    pub threshold_direction: Option<ThresholdDirection>,
}

/// Parameters for deleting a holding note (HNO-021).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DeleteHoldingNoteDTO {
    /// Account the note belongs to.
    pub account_id: String,
    /// The noted asset (identifies the note together with `account_id`).
    pub asset_id: String,
}

/// Creates or fully replaces the note for an (account, asset) pair (HNO-020).
#[tauri::command]
#[specta::specta]
pub async fn upsert_holding_note(
    state: State<'_, AppState>,
    dto: UpsertHoldingNoteDTO,
) -> Result<HoldingNote, AccountError> {
    state
        .account_service
        .upsert_holding_note(
            &dto.account_id,
            dto.asset_id,
            dto.text,
            dto.threshold_price,
            dto.threshold_direction,
        )
        .await
}

/// Deletes the note for an (account, asset) pair (HNO-021, silent if absent).
#[tauri::command]
#[specta::specta]
pub async fn delete_holding_note(
    state: State<'_, AppState>,
    dto: DeleteHoldingNoteDTO,
) -> Result<(), AccountError> {
    state
        .account_service
        .delete_holding_note(&dto.account_id, &dto.asset_id)
        .await
}
