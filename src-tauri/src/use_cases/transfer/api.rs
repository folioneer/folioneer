// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::error::TransferError;
use super::orchestrator::{TransferFigures, TransferUseCase};
use crate::context::account::{Transaction, TransferView};
use crate::shared::domain::quantity::from_surface;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

/// Parameters for recording a transfer (TRF-011).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct TransferDTO {
    /// The account the holding leaves.
    pub from_account_id: String,
    /// The account the holding reaches — another one (TRF-010).
    pub to_account_id: String,
    /// The asset moved — not a Cash Asset, not archived (TRF-010).
    pub asset_id: String,
    /// Business date (YYYY-MM-DD, TRF-011).
    pub date: String,
    /// Quantity leaving the sending account (micro-units, strictly positive).
    pub quantity_sent: i64,
    /// Quantity reaching the receiving account (micro-units, strictly positive, not above
    /// the quantity sent); the difference is the fee in kind (TRF-021).
    pub quantity_received: i64,
    /// Rate from the sending account's currency to the receiving one's (micros); `null`
    /// takes the stored rate of the date (TRF-022).
    pub exchange_rate: Option<i64>,
    /// Optional note, carried by every part.
    pub note: Option<String>,
}

/// Parameters for correcting a transfer (TRF-030); its accounts and asset are fixed.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CorrectTransferDTO {
    /// The transfer corrected.
    pub transfer_id: String,
    /// Corrected business date (YYYY-MM-DD).
    pub date: String,
    /// Corrected quantity sent (micro-units).
    pub quantity_sent: i64,
    /// Corrected quantity received (micro-units).
    pub quantity_received: i64,
    /// Corrected rate (micros); `null` takes the stored rate of the date again (TRF-030).
    pub exchange_rate: Option<i64>,
    /// Corrected note.
    pub note: Option<String>,
}

/// Records a transfer between two accounts at its cost (TRF-020); returns its parts.
#[tauri::command]
#[specta::specta]
pub async fn record_transfer(
    uc: State<'_, TransferUseCase>,
    dto: TransferDTO,
) -> Result<Vec<Transaction>, TransferError> {
    uc.record_transfer(
        &dto.from_account_id,
        &dto.to_account_id,
        dto.asset_id,
        TransferFigures {
            date: dto.date,
            quantity_sent: from_surface(dto.quantity_sent),
            quantity_received: from_surface(dto.quantity_received),
            exchange_rate: dto.exchange_rate,
            note: dto.note,
        },
    )
    .await
    .map(|parts| parts.iter().map(Transaction::as_shown).collect())
}

/// Corrects a transfer as one operation (TRF-030); returns its parts.
#[tauri::command]
#[specta::specta]
pub async fn correct_transfer(
    uc: State<'_, TransferUseCase>,
    dto: CorrectTransferDTO,
) -> Result<Vec<Transaction>, TransferError> {
    uc.correct_transfer(
        &dto.transfer_id,
        TransferFigures {
            date: dto.date,
            quantity_sent: from_surface(dto.quantity_sent),
            quantity_received: from_surface(dto.quantity_received),
            exchange_rate: dto.exchange_rate,
            note: dto.note,
        },
    )
    .await
    .map(|parts| parts.iter().map(Transaction::as_shown).collect())
}

/// Removes every part of a transfer together (TRF-040).
#[tauri::command]
#[specta::specta]
pub async fn cancel_transfer(
    uc: State<'_, TransferUseCase>,
    transfer_id: String,
) -> Result<(), TransferError> {
    uc.cancel_transfer(&transfer_id).await
}

/// A transfer read as one from its parts (TRF-060).
#[tauri::command]
#[specta::specta]
pub async fn get_transfer(
    uc: State<'_, TransferUseCase>,
    transfer_id: String,
) -> Result<TransferView, TransferError> {
    uc.get_transfer(&transfer_id)
        .await
        .map(TransferView::as_shown)
}

/// The rate a transfer between two accounts on a date takes when none is given (TRF-061);
/// `null` when none is stored.
#[tauri::command]
#[specta::specta]
pub async fn get_transfer_rate(
    uc: State<'_, TransferUseCase>,
    from_account_id: String,
    to_account_id: String,
    date: String,
) -> Result<Option<i64>, TransferError> {
    uc.transfer_rate(&from_account_id, &to_account_id, &date)
        .await
}
