//! Recording by what a person named (CLI-011, CLI-012, AGT-044): the account and the asset
//! found by name, the transaction recorded through the rules of the window, and the refusal
//! worded for whoever asked. The command line and the agent connection both record through
//! it, so the two never differ.

use serde::Serialize;

use crate::context::account::{AccountError, AgentRecording, Transaction};
use crate::core::BACKEND;

use super::error::{NameLookupError, OpenHoldingError, OpenHoldingTask};
use super::orchestrator::{HoldingTransactionUseCase, OpeningBalanceDraft};

const MICRO: i64 = 1_000_000;

/// What the user typed to name the account and the asset, and the shared values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The account's name as typed.
    pub account: String,
    /// The asset's name or reference as typed.
    pub asset: String,
    /// The date, or `None` for today.
    pub date: Option<String>,
    /// Quantity in micro-units.
    pub quantity: i64,
}

/// A purchase or a sale's money side: a unit price or the broker's all-in total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeAmount {
    /// Unit price in the asset's currency (micro-units).
    Price(i64),
    /// All-in total in the account's currency (micro-units).
    Total(i64),
}

/// A purchase or a sale as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    /// Account, asset, date and quantity.
    pub target: Target,
    /// Unit price or total.
    pub amount: TradeAmount,
    /// Fees in the account's currency (micro-units).
    pub fees: i64,
    /// Exchange rate (micro-units).
    pub rate: i64,
    /// Optional note.
    pub note: Option<String>,
}

/// A command that records a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recording {
    /// An opening balance: quantity and total cost.
    Open {
        /// Account, asset, date and quantity.
        target: Target,
        /// What the position cost in all, in the account's currency (micro-units).
        total_cost: i64,
    },
    /// A purchase.
    Buy(Trade),
    /// A sale.
    Sell(Trade),
}

/// A refusal: a stable code a script can test and a sentence for a person (CLI-021).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The code: the recording's own, or the asking interface's.
    pub code: String,
    /// The reason in plain English, naming only what the user typed.
    pub message: String,
}

/// What was recorded, with what names it for a person.
#[derive(Debug, Clone)]
pub struct Recorded {
    /// The transaction as recorded.
    pub transaction: Box<Transaction>,
    /// The account's name as stored.
    pub account_name: String,
    /// The account's currency.
    pub currency: String,
    /// The asset's reference.
    pub asset_reference: String,
    /// An opening balance of zero cost (TRX-065).
    pub zero_cost: bool,
}

impl HoldingTransactionUseCase {
    /// CLI-011 / CLI-012 — records what a person named: the account and the asset found by
    /// what was typed, then the transaction through the rules of the window. `today` is the
    /// date used when the recording gives none.
    pub async fn record_named(&self, command: Recording, today: &str) -> Result<Recorded, Refusal> {
        let target = match &command {
            Recording::Open { target, .. } => target,
            Recording::Buy(trade) | Recording::Sell(trade) => &trade.target,
        };
        let found = match self.find_by_name(&target.account, &target.asset).await {
            Ok(found) => found,
            Err(error) => return Err(refusal_from_lookup(&error)),
        };
        let date = target.date.clone().unwrap_or_else(|| today.to_string());
        // TRX-065 — the core says whether an opening balance calls for the warning.
        let zero_cost = match &command {
            Recording::Open { target, total_cost } => self
                .validate_opening_balance_draft(&OpeningBalanceDraft {
                    account_id: found.account_id.clone(),
                    asset_id: found.asset_id.clone(),
                    date: date.clone(),
                    quantity: target.quantity,
                    total_cost: Some(*total_cost),
                })
                .is_ok_and(|preview| preview.zero_cost),
            Recording::Buy(_) | Recording::Sell(_) => false,
        };
        let result = match command {
            Recording::Open { target, total_cost } => self
                .open_holding(
                    &found.account_id,
                    found.asset_id.clone(),
                    date,
                    target.quantity,
                    total_cost,
                )
                .await
                .map_err(|error| refusal_from_open(&error)),
            Recording::Buy(trade) => {
                let (unit_price, total) = split(&trade);
                self.buy_holding(
                    &found.account_id,
                    found.asset_id.clone(),
                    date,
                    trade.target.quantity,
                    unit_price,
                    trade.rate,
                    trade.fees,
                    total,
                    trade.note,
                )
                .await
                .map_err(|error| refusal_from_account(&error))
            }
            Recording::Sell(trade) => {
                let (unit_price, total) = split(&trade);
                self.sell_holding(
                    &found.account_id,
                    found.asset_id.clone(),
                    date,
                    trade.target.quantity,
                    unit_price,
                    trade.rate,
                    trade.fees,
                    total,
                    trade.note,
                )
                .await
                .map_err(|error| refusal_from_account(&error))
            }
        };
        result.map(|transaction| Recorded {
            transaction: Box::new(transaction),
            account_name: found.account_name,
            currency: found.currency,
            asset_reference: found.asset_reference,
            zero_cost,
        })
    }

    /// AGT-044 / AGT-049 — records what an agent named, marked with the agent and its
    /// session. When the mark cannot be written the recording is undone, and the refusal
    /// says so.
    pub async fn record_named_by_agent(
        &self,
        command: Recording,
        today: &str,
        mark: &AgentRecording,
    ) -> Result<Recorded, Refusal> {
        let recorded = self.record_named(command, today).await?;
        let transaction = &recorded.transaction;
        let Err(error) = self
            .account_service
            .mark_agent_recording(&transaction.id, mark)
            .await
        else {
            return Ok(recorded);
        };
        tracing::error!(target: BACKEND, transaction = %transaction.id, err = ?error, "agent recording: not marked, undoing it");
        match self
            .cancel_transaction(&transaction.account_id, &transaction.id)
            .await
        {
            Ok(()) => Err(Refusal {
                code: "NotRecorded".to_string(),
                message: "the recording could not be marked as an agent's, so nothing was recorded: try again".to_string(),
            }),
            Err(error) => {
                tracing::error!(target: BACKEND, transaction = %transaction.id, err = ?error, "agent recording: not marked and not undone");
                Err(Refusal {
                    code: "RecordedNotMarked".to_string(),
                    message: "the transaction was recorded, but could not be marked as recorded by an agent: tell the owner".to_string(),
                })
            }
        }
    }
}

/// A figure in micro-units as a decimal without trailing zeros (`2`, `992.19`, `0.5`).
pub fn decimal(micros: i64) -> String {
    let sign = if micros < 0 { "-" } else { "" };
    let absolute = micros.unsigned_abs();
    let whole = absolute / 1_000_000;
    let fraction = absolute % 1_000_000;
    if fraction == 0 {
        return format!("{sign}{whole}");
    }
    let fraction = format!("{fraction:06}");
    format!("{sign}{whole}.{}", fraction.trim_end_matches('0'))
}

/// A decimal with a dot and at most six decimals, in micro-units; `None` when it is not one.
/// A leading minus is read, so the core refuses a negative figure with its own reason.
pub fn decimal_to_micro(text: &str) -> Option<i64> {
    let text = text.trim();
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    if whole.is_empty() && fraction.is_empty()
        || fraction.len() > 6
        || !whole.chars().all(|c| c.is_ascii_digit())
        || !fraction.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction: i64 = format!("{fraction:0<6}").parse().ok()?;
    let micros = whole.checked_mul(MICRO)?.checked_add(fraction)?;
    Some(if negative { -micros } else { micros })
}

/// CLI-011 — a name the use case could not resolve, naming only what was typed.
fn refusal_from_lookup(error: &NameLookupError) -> Refusal {
    let message = match error {
        NameLookupError::AccountNotFound { typed } => format!("no account named \"{typed}\""),
        NameLookupError::AccountAmbiguous { typed } => {
            format!("more than one account is named \"{typed}\"")
        }
        NameLookupError::AssetNotFound { typed } => {
            format!("no asset matches \"{typed}\" by name, reference or ISIN")
        }
        NameLookupError::AssetAmbiguous { typed } => {
            format!(
                "\"{typed}\" matches more than one asset — use its name, its ISIN, or its reference with its exchange (ASML@XAMS)"
            )
        }
        NameLookupError::DatabaseError => "the portfolio could not be read".to_string(),
    };
    Refusal {
        code: code_of(error),
        message,
    }
}

/// The unit price and the typed total the recording command takes: a total leaves the unit
/// price to the core (TRX-060, SEL-050).
fn split(trade: &Trade) -> (i64, Option<i64>) {
    match trade.amount {
        TradeAmount::Price(price) => (price, None),
        TradeAmount::Total(total) => (0, Some(total)),
    }
}

/// The code a rejection carries on the wire: its serde tag (CLI-021).
pub fn code_of(error: &impl Serialize) -> String {
    serde_json::to_value(error)
        .ok()
        .and_then(|value| {
            value
                .get("code")
                .and_then(|code| code.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Unknown".to_string())
}

/// CLI-021 — an opening balance's rejection keeps its code; the sentence is the command line's.
fn refusal_from_open(error: &OpenHoldingError) -> Refusal {
    match error {
        OpenHoldingError::Account(error) => refusal_from_account(error),
        OpenHoldingError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                OpenHoldingTask::AssetNotFound => "the asset no longer exists".to_string(),
                OpenHoldingTask::ArchivedAsset => "the asset is archived".to_string(),
                OpenHoldingTask::OpeningBalanceOnCashAsset => {
                    "an opening balance cannot be recorded on cash".to_string()
                }
            },
        },
    }
}

/// CLI-021 — a recording rejection keeps its code; the sentence is the command line's. Every
/// variant is named, so a new one fails to compile until it has a sentence.
fn refusal_from_account(error: &AccountError) -> Refusal {
    let message = match error {
        AccountError::Oversell {
            available,
            requested,
        } => format!(
            "only {} held, not {}",
            decimal(*available),
            decimal(*requested)
        ),
        AccountError::InsufficientCash { .. } => {
            "not enough cash in the account for this purchase".to_string()
        }
        AccountError::ClosedPosition => "nothing of this asset is held in the account".to_string(),
        AccountError::CascadingOversell => {
            "this would leave a later sale above the quantity held".to_string()
        }
        AccountError::OpeningBalanceOnCashAsset => {
            "an opening balance cannot be recorded on cash".to_string()
        }
        AccountError::InvalidDate => "the date is not a valid date".to_string(),
        AccountError::DateInFuture => "the date is in the future".to_string(),
        AccountError::DateTooOld => "the date is before 1900".to_string(),
        AccountError::QuantityNotPositive => "the quantity must be above zero".to_string(),
        AccountError::UnitPriceNegative => "the price cannot be negative".to_string(),
        AccountError::UnitPriceOutOfRange => {
            "the price this total gives cannot be stored".to_string()
        }
        AccountError::FeesNegative => "the fees cannot be negative".to_string(),
        AccountError::ExchangeRateNotPositive => "the exchange rate must be above zero".to_string(),
        AccountError::TotalAmountNotPositive => "the total must be above zero".to_string(),
        AccountError::TotalAmountBelowFees => "the total is below the fees it includes".to_string(),
        AccountError::InvalidTotalCost => "the total cost cannot be negative".to_string(),
        AccountError::AccountNotFound { .. } => "the account no longer exists".to_string(),
        AccountError::DatabaseError | AccountError::ApplicationWriteOutranked => {
            "the portfolio could not be read or written".to_string()
        }
        // Rejections of other operations, which an opening balance, a purchase or a sale
        // never meets.
        AccountError::NameEmpty
        | AccountError::InvalidCurrency { .. }
        | AccountError::NegativeQuantity
        | AccountError::NegativeAveragePrice
        | AccountError::TransactionNotFound
        | AccountError::AmountNotPositive
        | AccountError::SplitFactorNotPositive
        | AccountError::SplitFactorIsOne
        | AccountError::SplitOnCashAsset
        | AccountError::SplitCollapsesPosition
        | AccountError::PercentageNotPositive
        | AccountError::PercentageAboveHundred
        | AccountError::ManagementFeeAmountInvalid
        | AccountError::ResultingQuantityNegative
        | AccountError::ResultingQuantityNotBelowHeld { .. }
        | AccountError::InterestAmountInvalid
        | AccountError::RateNotPositive
        | AccountError::RateAboveHundred
        | AccountError::EndBeforeStart
        | AccountError::ScheduleAlreadyExists
        | AccountError::ScheduleNotFound
        | AccountError::ManagementFeesDisabled
        | AccountError::NoteTextEmpty
        | AccountError::NoteTextTooLong
        | AccountError::ThresholdNotPositive
        | AccountError::ThresholdIncomplete
        | AccountError::NoteOnCashAsset
        | AccountError::TradeOnCashAsset
        | AccountError::NoteOnUnheldAsset
        | AccountError::NameAlreadyExists => format!("refused by the rules ({})", code_of(error)),
    };
    Refusal {
        code: code_of(error),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // CLI-021 — a recording rejection keeps its code; the sentence names the reason.
    #[test]
    fn cli_021_recording_rejections_keep_their_code() {
        let cases = [
            (
                AccountError::Oversell {
                    available: 1_000_000,
                    requested: 2_500_000,
                },
                "Oversell",
                "only 1 held, not 2.5",
            ),
            (
                AccountError::ClosedPosition,
                "ClosedPosition",
                "nothing of this asset is held in the account",
            ),
            (
                AccountError::CascadingOversell,
                "CascadingOversell",
                "this would leave a later sale above the quantity held",
            ),
            (
                AccountError::DateInFuture,
                "DateInFuture",
                "the date is in the future",
            ),
            (
                AccountError::DateTooOld,
                "DateTooOld",
                "the date is before 1900",
            ),
            (
                AccountError::InvalidDate,
                "InvalidDate",
                "the date is not a valid date",
            ),
            (
                AccountError::QuantityNotPositive,
                "QuantityNotPositive",
                "the quantity must be above zero",
            ),
            (
                AccountError::UnitPriceNegative,
                "UnitPriceNegative",
                "the price cannot be negative",
            ),
            (
                AccountError::UnitPriceOutOfRange,
                "UnitPriceOutOfRange",
                "the price this total gives cannot be stored",
            ),
            (
                AccountError::FeesNegative,
                "FeesNegative",
                "the fees cannot be negative",
            ),
            (
                AccountError::ExchangeRateNotPositive,
                "ExchangeRateNotPositive",
                "the exchange rate must be above zero",
            ),
            (
                AccountError::TotalAmountNotPositive,
                "TotalAmountNotPositive",
                "the total must be above zero",
            ),
            (
                AccountError::TotalAmountBelowFees,
                "TotalAmountBelowFees",
                "the total is below the fees it includes",
            ),
            (
                AccountError::InvalidTotalCost,
                "InvalidTotalCost",
                "the total cost cannot be negative",
            ),
            (
                AccountError::OpeningBalanceOnCashAsset,
                "OpeningBalanceOnCashAsset",
                "an opening balance cannot be recorded on cash",
            ),
            (
                AccountError::DatabaseError,
                "DatabaseError",
                "the portfolio could not be read or written",
            ),
            (
                AccountError::NameEmpty,
                "NameEmpty",
                "refused by the rules (NameEmpty)",
            ),
        ];
        for (error, code, message) in cases {
            let refusal = refusal_from_account(&error);
            assert_eq!(
                (refusal.code.as_str(), refusal.message.as_str()),
                (code, message)
            );
        }
    }

    // CLI-021 — an opening balance's own rejections keep their code too.
    #[test]
    fn cli_021_opening_balance_rejections_keep_their_code() {
        let archived =
            refusal_from_open(&OpenHoldingError::UseCase(OpenHoldingTask::ArchivedAsset));
        assert_eq!(
            (archived.code.as_str(), archived.message.as_str()),
            ("ArchivedAsset", "the asset is archived")
        );
        let gone = refusal_from_open(&OpenHoldingError::UseCase(OpenHoldingTask::AssetNotFound));
        assert_eq!(gone.code, "AssetNotFound");
        let cash = refusal_from_open(&OpenHoldingError::UseCase(
            OpenHoldingTask::OpeningBalanceOnCashAsset,
        ));
        assert_eq!(
            cash.message,
            "an opening balance cannot be recorded on cash"
        );
        let through = refusal_from_open(&OpenHoldingError::Account(AccountError::DateInFuture));
        assert_eq!(through.code, "DateInFuture");
    }

    // CLI-011 — a name the use case could not resolve keeps its code and names only what was
    // typed.
    #[test]
    fn cli_011_lookup_refusals_name_only_what_was_typed() {
        let typed = || "PEA".to_string();
        let cases = [
            (
                NameLookupError::AccountNotFound { typed: typed() },
                "AccountNotFound",
                "no account named \"PEA\"",
            ),
            (
                NameLookupError::AccountAmbiguous { typed: typed() },
                "AccountAmbiguous",
                "more than one account is named \"PEA\"",
            ),
            (
                NameLookupError::AssetNotFound { typed: typed() },
                "AssetNotFound",
                "no asset matches \"PEA\" by name, reference or ISIN",
            ),
            (
                NameLookupError::AssetAmbiguous { typed: typed() },
                "AssetAmbiguous",
                "\"PEA\" matches more than one asset — use its name, its ISIN, or its reference with its exchange (ASML@XAMS)",
            ),
            (
                NameLookupError::DatabaseError,
                "DatabaseError",
                "the portfolio could not be read",
            ),
        ];
        for (error, code, message) in cases {
            let refusal = refusal_from_lookup(&error);
            assert_eq!(
                (refusal.code.as_str(), refusal.message.as_str()),
                (code, message)
            );
        }
    }
}
