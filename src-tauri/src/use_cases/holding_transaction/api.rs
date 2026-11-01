// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::error::{DividendError, OpenHoldingError};
use super::{
    DraftKind, HoldingTransactionUseCase, OpeningBalanceDraft, OpeningBalancePreview,
    StockSplitDraft, StockSplitPreview, TransactionDraft, TransactionDraftError,
    TransactionPreview,
};
use crate::context::account::{
    AccountError, EnteredAmount, ManagementFeeRemoval, TransactionView, WriteOffReason,
};
use crate::shared::domain::quantity::{
    read_scaled, stored_non_cash_quantity, stored_quantity, MONEY_DECIMALS,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

/// TRX-024B — a quantity the window sent as decimal text, as stored.
fn read_quantity(asset_id: &str, text: &str) -> Result<i64, AccountError> {
    Ok(stored_quantity(asset_id, text)?)
}

// =============================================================================
// Opening Balance — DTO + dedicated error
// =============================================================================

/// Parameters for recording an opening balance for an asset in an account (TRX-042).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OpenHoldingDTO {
    /// Account where the opening balance is recorded.
    pub account_id: String,
    /// Financial asset being seeded.
    pub asset_id: String,
    /// Date of the opening balance (YYYY-MM-DD).
    pub date: String,
    /// Quantity as decimal text; strictly positive (TRX-044).
    pub quantity: String,
    /// Total cost paid in account currency (micro-units); strictly positive (TRX-045).
    pub total_cost: i64,
}

// =============================================================================
// Buy / Sell / Correct — DTOs (shared AccountError composite)
// =============================================================================

/// Parameters for recording a purchase of an asset into an account.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BuyHoldingDTO {
    /// Account where the purchase is recorded.
    pub account_id: String,
    /// Financial asset being purchased.
    pub asset_id: String,
    /// Transaction date (YYYY-MM-DD).
    pub date: String,
    /// Quantity as decimal text.
    pub quantity: String,
    /// Unit price in asset currency (micro-units).
    pub unit_price: i64,
    /// Exchange rate asset→account currency (micro-units).
    pub exchange_rate: i64,
    /// Fees in account currency (micro-units).
    pub fees: i64,
    /// All-in total debited by the broker in account currency (micro-units),
    /// fees included (TRX-060). When provided it is stored verbatim and
    /// `unit_price` is derived from it.
    pub total_amount: Option<i64>,
    /// Optional user note.
    pub note: Option<String>,
}

/// Parameters for recording a sale of an asset from an account.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SellHoldingDTO {
    /// Account where the sale is recorded.
    pub account_id: String,
    /// Financial asset being sold.
    pub asset_id: String,
    /// Transaction date (YYYY-MM-DD).
    pub date: String,
    /// Quantity as decimal text.
    pub quantity: String,
    /// Unit price in asset currency (micro-units).
    pub unit_price: i64,
    /// Exchange rate asset→account currency (micro-units).
    pub exchange_rate: i64,
    /// Fees in account currency (micro-units).
    pub fees: i64,
    /// All-in net proceeds credited by the broker in account currency
    /// (micro-units), after fees (SEL-050). When provided it is stored verbatim
    /// and `unit_price` is derived from it.
    pub total_amount: Option<i64>,
    /// Optional user note.
    pub note: Option<String>,
}

/// Parameters for correcting an existing transaction.
/// `asset_id` is immutable — taken from the existing transaction.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CorrectTransactionDTO {
    /// Account that owns the transaction being corrected.
    pub account_id: String,
    /// Identifier of the transaction being corrected.
    pub transaction_id: String,
    /// Corrected transaction date (YYYY-MM-DD).
    pub date: String,
    /// Corrected quantity, as decimal text (TRX-024B).
    pub quantity: String,
    /// Corrected unit price in asset currency (micro-units).
    pub unit_price: i64,
    /// Corrected exchange rate asset→account currency (micro-units).
    pub exchange_rate: i64,
    /// Corrected fees in account currency (micro-units).
    pub fees: i64,
    /// All-in total typed by the user in account currency (micro-units) —
    /// TRX-061 / SEL-051 / TRX-051. When provided on a Purchase, Sell or
    /// OpeningBalance correction it is stored verbatim and `unit_price` is derived
    /// from it; ignored on every other transaction type.
    pub total_amount: Option<i64>,
    /// Optional user note.
    pub note: Option<String>,
}

/// Parameters for cancelling an existing transaction.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CancelTransactionDTO {
    /// Account that owns the transaction being cancelled.
    pub account_id: String,
    /// Identifier of the transaction being cancelled.
    pub transaction_id: String,
}

// =============================================================================
// Commands
// =============================================================================

/// Seeds a holding directly from a known quantity and total cost (TRX-042, TRX-047).
#[tauri::command]
#[specta::specta]
pub async fn open_holding(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: OpenHoldingDTO,
) -> Result<TransactionView, OpenHoldingError> {
    let quantity = read_quantity(&dto.asset_id, &dto.quantity)?;
    uc.open_holding(
        &dto.account_id,
        dto.asset_id,
        dto.date,
        quantity,
        dto.total_cost,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

/// Records a purchase of an asset into an account (TRX-027).
#[tauri::command]
#[specta::specta]
pub async fn buy_holding(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: BuyHoldingDTO,
) -> Result<TransactionView, AccountError> {
    let quantity = read_quantity(&dto.asset_id, &dto.quantity)?;
    uc.buy_holding(
        &dto.account_id,
        dto.asset_id,
        dto.date,
        quantity,
        dto.unit_price,
        dto.exchange_rate,
        dto.fees,
        dto.total_amount,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

/// Records a sale of an asset from an account (SEL-012, SEL-021, SEL-023, SEL-024).
#[tauri::command]
#[specta::specta]
pub async fn sell_holding(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: SellHoldingDTO,
) -> Result<TransactionView, AccountError> {
    let quantity = read_quantity(&dto.asset_id, &dto.quantity)?;
    uc.sell_holding(
        &dto.account_id,
        dto.asset_id,
        dto.date,
        quantity,
        dto.unit_price,
        dto.exchange_rate,
        dto.fees,
        dto.total_amount,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

/// Corrects an existing transaction and recalculates the affected holding
/// (TRX-031, TRX-061, SEL-051).
#[tauri::command]
#[specta::specta]
pub async fn correct_transaction(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: CorrectTransactionDTO,
) -> Result<TransactionView, AccountError> {
    let quantity = uc
        .stored_correction_quantity(&dto.transaction_id, &dto.quantity)
        .await?;
    uc.correct_transaction(
        &dto.account_id,
        &dto.transaction_id,
        dto.date,
        quantity,
        dto.unit_price,
        dto.exchange_rate,
        dto.fees,
        dto.total_amount,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

/// Cancels a transaction and recalculates (or removes) the associated holding (TRX-034).
#[tauri::command]
#[specta::specta]
pub async fn cancel_transaction(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: CancelTransactionDTO,
) -> Result<(), AccountError> {
    uc.cancel_transaction(&dto.account_id, &dto.transaction_id)
        .await
}

// =============================================================================
// Cash Transactions — DTOs + dedicated errors (CSH-022 / CSH-032)
// =============================================================================

/// Parameters for recording a cash deposit (CSH-020).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DepositDTO {
    /// Account receiving the cash.
    pub account_id: String,
    /// Transaction date (YYYY-MM-DD).
    pub date: String,
    /// Deposited amount in account currency (micro-units); strictly positive (CSH-021).
    pub amount_micros: i64,
    /// Optional user note.
    pub note: Option<String>,
}

/// Parameters for recording a cash withdrawal (CSH-030).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct WithdrawalDTO {
    /// Account from which to withdraw cash.
    pub account_id: String,
    /// Transaction date (YYYY-MM-DD).
    pub date: String,
    /// Withdrawn amount in account currency (micro-units); strictly positive (CSH-031).
    pub amount_micros: i64,
    /// Optional user note.
    pub note: Option<String>,
}

/// Records a cash deposit into an account (CSH-022).
#[tauri::command]
#[specta::specta]
pub async fn record_deposit(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: DepositDTO,
) -> Result<TransactionView, AccountError> {
    uc.record_deposit(&dto.account_id, dto.date, dto.amount_micros, dto.note)
        .await
        .map(|transaction| TransactionView::from(&transaction))
}

/// Records a cash withdrawal from an account (CSH-032).
#[tauri::command]
#[specta::specta]
pub async fn record_withdrawal(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: WithdrawalDTO,
) -> Result<TransactionView, AccountError> {
    uc.record_withdrawal(&dto.account_id, dto.date, dto.amount_micros, dto.note)
        .await
        .map(|transaction| TransactionView::from(&transaction))
}

// =============================================================================
// Dividend — DTO + command (DIV-020/023)
// =============================================================================

/// Parameters for recording a cash dividend attributed to a held asset (DIV-020).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DividendDTO {
    /// Account receiving the dividend.
    pub account_id: String,
    /// The paying asset — must be actively held (quantity > 0) and not a Cash Asset (DIV-011).
    pub asset_id: String,
    /// Business date the dividend was received (YYYY-MM-DD, DIV-021).
    pub date: String,
    /// Net dividend in the asset's native currency (micro-units, strictly positive, DIV-021).
    pub amount_micros: i64,
    /// Asset→account conversion rate (micro-units, strictly positive; 1_000_000 when currencies match, DIV-022).
    pub exchange_rate: i64,
    /// Optional user note.
    pub note: Option<String>,
}

/// Records a cash dividend attributed to a held asset (DIV-023).
#[tauri::command]
#[specta::specta]
pub async fn record_dividend(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: DividendDTO,
) -> Result<TransactionView, DividendError> {
    uc.record_dividend(
        &dto.account_id,
        dto.asset_id,
        dto.date,
        dto.amount_micros,
        dto.exchange_rate,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

// =============================================================================
// Free Shares — DTO + command (FSD-020/022)
// =============================================================================

/// Parameters for recording a zero-cost free-share distribution from a held
/// distributing asset (FSD-020). No amount, no unit price, no exchange rate,
/// no fees — no money changes hands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FreeSharesDTO {
    /// Account whose holding receives the free shares.
    pub account_id: String,
    /// The distributing asset — must be actively held (quantity > 0) and not a Cash Asset (FSD-011).
    pub asset_id: String,
    /// Business date the shares were received (YYYY-MM-DD, FSD-021).
    pub date: String,
    /// Number of free shares received (decimal text, strictly positive, FSD-021).
    pub quantity: String,
    /// Optional user note.
    pub note: Option<String>,
}

/// Records a zero-cost free-share distribution attributed to a held asset (FSD-022).
#[tauri::command]
#[specta::specta]
pub async fn record_free_shares(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: FreeSharesDTO,
) -> Result<TransactionView, super::error::FreeSharesError> {
    let quantity = read_quantity(&dto.asset_id, &dto.quantity)?;
    uc.record_free_shares(&dto.account_id, dto.asset_id, dto.date, quantity, dto.note)
        .await
        .map(|transaction| TransactionView::from(&transaction))
}

// =============================================================================
// Write-off — DTOs + commands (WOF-056)
// =============================================================================

/// Parameters for recording a write-off: a quantity that leaves a holding without
/// proceeds (WOF-056). No amount, no unit price, no exchange rate, no fees.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct WriteOffDTO {
    /// Account whose holding the quantity leaves.
    pub account_id: String,
    /// The asset written off — held on the date, not a Cash Asset, not archived (WOF-010).
    pub asset_id: String,
    /// Business date the quantity left (YYYY-MM-DD, WOF-011).
    pub date: String,
    /// Quantity written off (decimal text, strictly positive, WOF-011).
    pub quantity: String,
    /// Why it left (WOF entity).
    pub reason: WriteOffReason,
    /// Optional user note.
    pub note: Option<String>,
}

/// Records a write-off through the use case the agent connection uses (WOF-056, AGT-065).
#[tauri::command]
#[specta::specta]
pub async fn record_write_off(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: WriteOffDTO,
) -> Result<TransactionView, super::error::WriteOffError> {
    let quantity = read_quantity(&dto.asset_id, &dto.quantity)?;
    uc.record_write_off(
        &dto.account_id,
        dto.asset_id,
        dto.date,
        quantity,
        dto.reason,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

/// Parameters for correcting a write-off (WOF-030, WOF-058); the asset is fixed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CorrectWriteOffDTO {
    /// Account owning the write-off.
    pub account_id: String,
    /// The write-off corrected.
    pub transaction_id: String,
    /// Corrected business date (YYYY-MM-DD, WOF-011).
    pub date: String,
    /// Corrected quantity (decimal text, strictly positive, WOF-011).
    pub quantity: String,
    /// Corrected reason (WOF entity).
    pub reason: WriteOffReason,
    /// Corrected note.
    pub note: Option<String>,
}

/// Corrects a write-off's date, quantity, reason and note, the holding replayed (WOF-030).
#[tauri::command]
#[specta::specta]
pub async fn correct_write_off(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: CorrectWriteOffDTO,
) -> Result<TransactionView, AccountError> {
    uc.correct_write_off(
        &dto.account_id,
        &dto.transaction_id,
        dto.date,
        stored_non_cash_quantity(&dto.quantity).map_err(AccountError::from)?,
        dto.reason,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

// =============================================================================
// Stock Split — DTO + command (SPL-010)
// =============================================================================

/// Parameters for recording a stock split on a held asset (SPL-010).
#[derive(serde::Deserialize, specta::Type)]
pub struct RecordSplitDTO {
    /// Account whose position is rescaled.
    pub account_id: String,
    /// The split asset — must be actively held (quantity > 0) and not a Cash Asset (SPL-012).
    pub asset_id: String,
    /// Effective date of the split (YYYY-MM-DD, not in the future).
    pub date: String,
    /// Micro-scaled split factor (20-for-1 → 20_000_000; 1-for-10 → 100_000), strictly positive and ≠ ×1 (SPL-011).
    pub factor: i64,
    /// Optional user note.
    pub note: Option<String>,
}

/// Records a stock split rescaling a held position at its date (SPL-010/020).
#[tauri::command]
#[specta::specta]
pub async fn record_split(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: RecordSplitDTO,
) -> Result<TransactionView, super::error::SplitError> {
    uc.record_split(
        &dto.account_id,
        dto.asset_id,
        dto.date,
        dto.factor,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

// =============================================================================
// Management Fee — DTO + command (FEE-020/022)
// =============================================================================

/// Parameters for recording a one-off management fee on a held asset (FEE-020).
/// The fee is either a percentage of the holding or the quantity the holding should
/// hold after it — exactly one of the two (FEE-021); no money changes hands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ManagementFeeDTO {
    /// Account whose holding the fee is taken from.
    pub account_id: String,
    /// The charged asset — must be actively held (quantity > 0) and not a Cash Asset (FEE-012).
    pub asset_id: String,
    /// Business date the fee was applied (YYYY-MM-DD, FEE-021).
    pub date: String,
    /// Percentage of the holding to remove, in micro-percent (1% = 1_000_000),
    /// strictly positive and at most 100_000_000; mutually exclusive with
    /// `resulting_quantity` (FEE-021).
    pub percent_micros: Option<i64>,
    /// Quantity the holding should hold after the fee, as of `date` (decimal text, zero
    /// or more and below the held quantity); mutually exclusive with `percent_micros`
    /// (FEE-021/028).
    pub resulting_quantity: Option<String>,
    /// Optional user note.
    pub note: Option<String>,
}

/// Records a one-off quantity-reducing management fee on a held asset (FEE-022/028).
#[tauri::command]
#[specta::specta]
pub async fn record_management_fee(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: ManagementFeeDTO,
) -> Result<TransactionView, super::error::ManagementFeeError> {
    let resulting = dto
        .resulting_quantity
        .as_deref()
        .map(|text| read_quantity(&dto.asset_id, text))
        .transpose()?;
    uc.record_management_fee_entry(
        &dto.account_id,
        dto.asset_id,
        dto.date,
        dto.percent_micros,
        resulting,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

/// What a one-off management fee entered by its resulting quantity would remove
/// (FEE-029): the quantity held as of `date`, the removal and the percentage it represents.
#[tauri::command]
#[specta::specta]
pub async fn preview_management_fee(
    uc: State<'_, HoldingTransactionUseCase>,
    account_id: String,
    asset_id: String,
    date: String,
    resulting_quantity: String,
) -> Result<ManagementFeeRemoval, super::error::ManagementFeeError> {
    let resulting = read_quantity(&asset_id, &resulting_quantity)?;
    uc.preview_management_fee(&account_id, asset_id, date, resulting)
        .await
}

// =============================================================================
// Interest — DTO + command (INT-020/021)
// =============================================================================

/// Parameters for recording an interest credit on a held asset or the account's
/// cash line (INT-020). The amount is either a rate or a direct quantity —
/// exactly one of the two must be provided (INT-021). No money changes hands.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RecordInterestDTO {
    /// Account whose holding (or cash line) receives the interest.
    pub account_id: String,
    /// The credited asset — a currently held non-cash asset or the account's Cash Asset (INT-011/023).
    pub asset_id: String,
    /// Business date the interest was credited (YYYY-MM-DD, INT-021).
    pub date: String,
    /// Interest rate in micro-percent (1% = 1_000_000), strictly positive and at
    /// most 100_000_000; mutually exclusive with `quantity` (INT-021/022).
    pub percent_micros: Option<i64>,
    /// Credited quantity (decimal text, strictly positive); mutually exclusive
    /// with `percent_micros` (INT-021).
    pub quantity: Option<String>,
    /// Optional user note.
    pub note: Option<String>,
}

/// Records a zero-cost interest credit on a held asset or the cash line (INT-023/024).
#[tauri::command]
#[specta::specta]
pub async fn record_interest(
    uc: State<'_, HoldingTransactionUseCase>,
    dto: RecordInterestDTO,
) -> Result<TransactionView, super::error::InterestError> {
    // INT-023 — on the account's cash line the credit is money, on its own scale.
    let credited = dto
        .quantity
        .as_deref()
        .map(|text| read_quantity(&dto.asset_id, text))
        .transpose()?;
    uc.record_interest(
        &dto.account_id,
        dto.asset_id,
        dto.date,
        dto.percent_micros,
        credited,
        dto.note,
    )
    .await
    .map(|transaction| TransactionView::from(&transaction))
}

/// A transaction draft as the window sends it: the quantity as decimal text (TRX-024B); a
/// dividend's draft carries its amount there, which is money (TRX-062).
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(rename = "TransactionDraft")]
pub struct TransactionDraftDTO {
    /// Purchase, sale or dividend.
    pub kind: DraftKind,
    /// The account, empty until chosen.
    pub account_id: String,
    /// The asset, empty until chosen.
    pub asset_id: String,
    /// ISO date, empty until entered.
    pub date: String,
    /// Quantity, as decimal text; `0` until entered.
    pub quantity: String,
    /// What the user entered: a unit price or a typed total, with rate and fees.
    pub entered: EnteredAmount,
    /// The transaction being corrected, if any.
    pub correcting: Option<String>,
}

impl TransactionDraftDTO {
    fn into_draft(self) -> Result<TransactionDraft, AccountError> {
        let quantity = match self.kind {
            DraftKind::Dividend => read_scaled(&self.quantity, MONEY_DECIMALS)?,
            DraftKind::Purchase | DraftKind::Sell => stored_non_cash_quantity(&self.quantity)?,
        };
        Ok(TransactionDraft {
            kind: self.kind,
            account_id: self.account_id,
            asset_id: self.asset_id,
            date: self.date,
            quantity,
            entered: self.entered,
            correcting: self.correcting,
        })
    }
}

/// An opening balance draft as the window sends it: the quantity as decimal text (TRX-024B).
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(rename = "OpeningBalanceDraft")]
pub struct OpeningBalanceDraftDTO {
    /// The account, empty until chosen.
    pub account_id: String,
    /// The asset, empty until chosen.
    pub asset_id: String,
    /// ISO date, empty until entered.
    pub date: String,
    /// Quantity, as decimal text; `0` until entered.
    pub quantity: String,
    /// Total cost in account currency, in micros; `None` until entered.
    pub total_cost: Option<i64>,
}

/// Checks a transaction draft without writing anything (TRX-062): the unit price and total
/// recording it would store, or the first problem as a code the form displays.
#[tauri::command]
#[specta::specta]
pub async fn validate_transaction_draft(
    uc: State<'_, HoldingTransactionUseCase>,
    draft: TransactionDraftDTO,
) -> Result<TransactionPreview, TransactionDraftError> {
    uc.validate_draft(draft.into_draft()?).await
}

/// Checks a split draft without writing anything (SPL-062): the factor recording would
/// store, the position before and after, the price to carry across the split — or the
/// first problem as a code.
#[tauri::command]
#[specta::specta]
pub async fn validate_stock_split_draft(
    uc: State<'_, HoldingTransactionUseCase>,
    draft: StockSplitDraft,
) -> Result<StockSplitPreview, TransactionDraftError> {
    uc.validate_stock_split_draft(draft).await
}

/// Checks an opening balance draft without writing anything (TRX-066): whether its total
/// cost of 0 calls for a warning (TRX-065), or the first problem as a code.
#[tauri::command]
#[specta::specta]
pub async fn validate_opening_balance_draft(
    uc: State<'_, HoldingTransactionUseCase>,
    draft: OpeningBalanceDraftDTO,
) -> Result<OpeningBalancePreview, TransactionDraftError> {
    let quantity = read_quantity(&draft.asset_id, &draft.quantity)?;
    uc.validate_opening_balance_draft(&OpeningBalanceDraft {
        account_id: draft.account_id,
        asset_id: draft.asset_id,
        date: draft.date,
        quantity,
        total_cost: draft.total_cost,
    })
}
