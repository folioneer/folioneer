// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::domain::{
    country_offers, generic_wrapper_ids, offered_countries, Account, FeeFrequency, FeeSchedule,
    HoldingNote, ThresholdDirection, Wrapper, PORTFOLIO_COUNTRY, WRAPPERS,
};
use crate::context::account::{
    AccountError, AccountJournal, HoldingSnapshotView, JournalFilter, TransactionView,
};
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
    /// Whether the % management-fee mechanism is enabled (FEE-075).
    pub management_fees_enabled: bool,
    /// ACC-041 — the account's own country, when it differs from the portfolio's.
    pub country: Option<String>,
    /// ACC-043 — the identifier of its tax wrapper (ACC-040); none when unset.
    pub wrapper: Option<String>,
}

/// ACC-040 — the wrapper lists, with the portfolio's country an account without its own takes.
#[derive(Debug, Serialize, Type)]
pub struct WrapperLists {
    /// The portfolio's country (ISO 3166-1 alpha-2): France until the first run asks it.
    pub portfolio_country: String,
    /// Every wrapper, the generic list and each country's, in the lists' order (ACC-045).
    pub wrappers: Vec<Wrapper>,
    /// ACC-045 — the countries the form offers, the portfolio's first.
    pub countries: Vec<String>,
    /// ACC-045 — for each country with a list of its own (and the portfolio's), its wrappers in
    /// the order the form offers them; any other country takes `generic`.
    pub offered: Vec<CountryWrappers>,
    /// ACC-045 — what a country with no list of its own is offered: the generic list.
    pub generic: Vec<String>,
}

/// ACC-045 — the wrappers one country is offered, in order.
#[derive(Debug, Serialize, Type)]
pub struct CountryWrappers {
    /// The country (ISO 3166-1 alpha-2).
    pub country: String,
    /// Its wrappers' identifiers: its own list, then the generic one.
    pub wrapper_ids: Vec<String>,
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
        .update_classified(
            dto.id,
            dto.name,
            dto.bank_name,
            dto.currency,
            dto.management_fees_enabled,
            dto.country,
            dto.wrapper,
        )
        .await
}

/// ACC-040 — the wrapper lists the account form offers; infallible, the lists are the
/// application's own.
#[tauri::command]
#[specta::specta]
pub fn get_wrapper_lists() -> WrapperLists {
    let owned = |ids: Vec<&str>| ids.into_iter().map(str::to_string).collect::<Vec<_>>();
    WrapperLists {
        portfolio_country: PORTFOLIO_COUNTRY.to_string(),
        wrappers: WRAPPERS.to_vec(),
        countries: owned(offered_countries()),
        offered: country_offers()
            .into_iter()
            .map(|(country, wrapper_ids)| CountryWrappers {
                country: country.to_string(),
                wrapper_ids: owned(wrapper_ids),
            })
            .collect(),
        generic: owned(generic_wrapper_ids()),
    }
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
) -> Result<Vec<TransactionView>, AccountError> {
    state
        .account_service
        .get_transactions(&account_id, &asset_id)
        .await
        .map(|transactions| transactions.iter().map(TransactionView::from).collect())
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
) -> Result<HoldingSnapshotView, AccountError> {
    state
        .account_service
        .holding_snapshot_as_of(&account_id, &asset_id, &date)
        .await
        .map(|snapshot| snapshot.as_view(&asset_id))
}

/// The account journal of an account: its transactions in date order, each with the cash
/// it moved and the cash balance after it, the rows `filter` keeps (TXL-060).
#[tauri::command]
#[specta::specta]
pub async fn get_account_journal(
    state: State<'_, AppState>,
    account_id: String,
    filter: JournalFilter,
) -> Result<AccountJournal, AccountError> {
    state
        .account_service
        .get_account_journal(&account_id, &filter)
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
