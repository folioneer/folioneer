//! Running a command (CLI-011, CLI-012): the account and asset found by what the user
//! typed, then recorded through the same use case the window calls.

use std::sync::Arc;

use serde::Serialize;

use crate::context::account::{AccountError, AccountServiceContract, Transaction, TransactionType};
use crate::context::asset::AssetServiceContract;
use crate::use_cases::holding_transaction::{
    HoldingTransactionUseCase, NameLookupError, OpenHoldingError, OpenHoldingTask,
};

use super::args::{Command, Trade, TradeAmount};

/// A refusal: a stable code a script can test and a sentence for a person (CLI-021).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The code: the recording command's, or the command line's own.
    pub code: String,
    /// The reason in plain English, naming only what the user typed.
    pub message: String,
}

/// What a command did.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// It recorded this transaction, in this account and currency, on this asset.
    Recorded {
        transaction: Box<Transaction>,
        account_name: String,
        currency: String,
        asset_reference: String,
    },
    /// It recorded nothing.
    Refused(Refusal),
}

/// CLI-011/012 — asks the holding use case for what a command names, then records through it:
/// the same use case the window calls. An interface adapter, like the Tauri commands.
pub struct CommandRunner {
    holding: HoldingTransactionUseCase,
}

impl CommandRunner {
    /// Wires the runner over the account and asset services.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
    ) -> Self {
        Self {
            holding: HoldingTransactionUseCase::new(account_service, asset_service),
        }
    }

    /// Runs a command; `today` is the date used when the command gives none.
    pub async fn run(&self, command: Command, today: &str) -> Outcome {
        let target = match &command {
            Command::Open { target, .. } => target,
            Command::Buy(trade) | Command::Sell(trade) => &trade.target,
        };
        let found = match self
            .holding
            .find_by_name(&target.account, &target.asset)
            .await
        {
            Ok(found) => found,
            Err(error) => return Outcome::Refused(refusal_from_lookup(&error)),
        };
        let date = target.date.clone().unwrap_or_else(|| today.to_string());
        let result = match command {
            Command::Open { target, total_cost } => self
                .holding
                .open_holding(
                    &found.account_id,
                    found.asset_id.clone(),
                    date,
                    target.quantity,
                    total_cost,
                )
                .await
                .map_err(|error| refusal_from_open(&error)),
            Command::Buy(trade) => {
                let (unit_price, total) = split(&trade);
                self.holding
                    .buy_holding(
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
            Command::Sell(trade) => {
                let (unit_price, total) = split(&trade);
                self.holding
                    .sell_holding(
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
        match result {
            Ok(transaction) => Outcome::Recorded {
                transaction: Box::new(transaction),
                account_name: found.account_name,
                currency: found.currency,
                asset_reference: found.asset_reference,
            },
            Err(refusal) => Outcome::Refused(refusal),
        }
    }
}

/// CLI-011 — a name the use case could not resolve, naming only what was typed.
fn refusal_from_lookup(error: &NameLookupError) -> Refusal {
    let message = match error {
        NameLookupError::AccountNotFound { typed } => format!("no account named \"{typed}\""),
        NameLookupError::AccountAmbiguous { typed } => {
            format!("more than one account is named \"{typed}\"")
        }
        NameLookupError::AssetNotFound { typed } => {
            format!("no asset matches \"{typed}\" by name or reference")
        }
        NameLookupError::AssetAmbiguous { typed } => {
            format!("\"{typed}\" matches more than one asset — use its reference")
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
fn code_of(error: &impl Serialize) -> String {
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
    use super::output::decimal;
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

/// Which way a recorded transaction reads in a sentence.
pub fn verb(transaction_type: TransactionType) -> &'static str {
    match transaction_type {
        TransactionType::OpeningBalance => "opened",
        TransactionType::Purchase => "bought",
        TransactionType::Sell => "sold",
        _ => "recorded",
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
                "no asset matches \"PEA\" by name or reference",
            ),
            (
                NameLookupError::AssetAmbiguous { typed: typed() },
                "AssetAmbiguous",
                "\"PEA\" matches more than one asset — use its reference",
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

    // CLI-020 — a recorded transaction reads with the verb of its type.
    #[test]
    fn cli_020_each_type_reads_with_its_verb() {
        assert_eq!(verb(TransactionType::OpeningBalance), "opened");
        assert_eq!(verb(TransactionType::Purchase), "bought");
        assert_eq!(verb(TransactionType::Sell), "sold");
        assert_eq!(verb(TransactionType::Dividend), "recorded");
    }
}
