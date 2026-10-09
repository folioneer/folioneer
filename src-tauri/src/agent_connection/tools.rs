//! Running the tools an agent may call (AGT-040), in the running application: each is a
//! query the window already makes.

use std::sync::Arc;

use serde_json::Value;

use crate::context::account::{AccountError, AccountServiceContract, AgentRecording};
use crate::context::asset::{Asset, AssetClass, AssetKind, AssetServiceContract};
use crate::context::currency::CurrencyService;
use crate::core::BACKEND;
use crate::use_cases::account_details::AccountDetailsUseCase;
use crate::use_cases::account_summary::AccountSummaryUseCase;
use crate::use_cases::holding_transaction::{
    decimal_to_micro, HoldingTransactionUseCase, Recording, Target, Trade, TradeAmount,
};

use super::connections::{Granted, MAX_RECORDINGS};

/// Why a tool call is refused: a stable code and a reason for the agent to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// A stable code for the reason.
    pub code: String,
    /// The reason; it names only what the agent sent (AGT-042).
    pub message: String,
}

/// An account as a tool names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct AccountRow {
    name: String,
    currency: String,
}

/// An asset as a tool names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
struct AssetRow {
    name: String,
    reference: String,
    kind: AssetKind,
    class: AssetClass,
    currency: String,
    isin: Option<String>,
    exchange: Option<String>,
    archived: bool,
}

impl From<Asset> for AssetRow {
    fn from(asset: Asset) -> Self {
        Self {
            name: asset.name,
            reference: asset.reference,
            kind: asset.kind,
            class: asset.class,
            currency: asset.currency,
            isin: asset.isin,
            exchange: asset.exchange.map(|exchange| exchange.code),
            archived: asset.is_archived,
        }
    }
}

/// Runs the tools in the running application, through the queries the window uses.
pub struct AgentTools {
    recorder: HoldingTransactionUseCase,
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    summaries: AccountSummaryUseCase,
    details: AccountDetailsUseCase,
}

impl AgentTools {
    /// Wires the tools over the application's services.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            recorder: HoldingTransactionUseCase::new(
                Arc::clone(&account_service),
                Arc::clone(&asset_service),
            ),
            summaries: AccountSummaryUseCase::new(
                Arc::clone(&account_service),
                Arc::clone(&asset_service),
                Arc::clone(&currency_service),
            ),
            details: AccountDetailsUseCase::new(
                Arc::clone(&account_service),
                Arc::clone(&asset_service),
                currency_service,
            ),
            account_service,
            asset_service,
        }
    }

    /// AGT-040 / AGT-044 — runs `tool` for the session `granted` and returns its result, or
    /// why it is refused. A refusal names only what the agent sent (AGT-042).
    pub async fn call(
        &self,
        granted: &Granted,
        tool: &str,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        match tool {
            "record_opening_balance" => {
                let recording = Recording::Open {
                    target: target(arguments)?,
                    total_cost: amount(arguments, "total_cost")?,
                };
                self.record(granted, recording).await
            }
            "record_purchase" => {
                self.record(granted, Recording::Buy(trade(arguments)?))
                    .await
            }
            "record_sale" => {
                self.record(granted, Recording::Sell(trade(arguments)?))
                    .await
            }
            "portfolio_summary" => self
                .summaries
                .get_account_summaries()
                .await
                .map_err(|error| unreadable("portfolio_summary", &error))
                .and_then(|summaries| as_json(&summaries)),
            "list_accounts" => {
                let accounts = self
                    .account_service
                    .get_all_by_name()
                    .await
                    .map_err(|error| unreadable("list_accounts", &error))?;
                let rows: Vec<AccountRow> = accounts
                    .into_iter()
                    .map(|account| AccountRow {
                        name: account.name,
                        currency: account.currency,
                    })
                    .collect();
                as_json(&rows)
            }
            "list_assets" => {
                let archived = arguments
                    .get("archived")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let assets = self
                    .asset_service
                    .get_non_cash_assets_by_name(archived)
                    .await
                    .map_err(|error| unreadable("list_assets", &error))?;
                let rows: Vec<AssetRow> = assets.into_iter().map(AssetRow::from).collect();
                as_json(&rows)
            }
            "list_holdings" => {
                let account = text(arguments, "account")?;
                let as_of = arguments.get("as_of").and_then(Value::as_str);
                let account_id = self.account_named(account).await?;
                self.details
                    .get_account_details(&account_id, as_of)
                    .await
                    .map_err(|error| match error {
                        AccountError::InvalidDate | AccountError::DateInFuture => Refusal {
                            code: "InvalidDate".to_string(),
                            message: "as_of is not a past date written YYYY-MM-DD".to_string(),
                        },
                        other => unreadable("list_holdings", &other),
                    })
                    .and_then(|details| as_json(&details))
            }
            _ => Err(Refusal {
                code: "UnknownTool".to_string(),
                message: "no such tool".to_string(),
            }),
        }
    }

    /// AGT-044 / AGT-045 — records through the rules of the window, marked with the agent
    /// and its session. The refusals are the recording's own (CLI-011, CLI-012).
    async fn record(&self, granted: &Granted, recording: Recording) -> Result<Value, Refusal> {
        if !granted.may_record_one_more() {
            return Err(Refusal {
                code: "SessionLimitReached".to_string(),
                message: format!(
                    "this session has recorded {MAX_RECORDINGS} transactions, its limit: the owner disconnects and allows a new session to go on"
                ),
            });
        }
        let today = chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let mark = AgentRecording {
            agent: granted.client.clone(),
            session: granted.key.clone(),
            session_started_at: granted.started_at.clone(),
        };
        let recorded = self
            .recorder
            .record_named_by_agent(recording, &today, &mark)
            .await
            .map_err(|refusal| Refusal {
                code: refusal.code,
                message: refusal.message,
            })?;
        granted.count_recording();
        tracing::info!(target: BACKEND, session = granted.session_id, transaction = %recorded.transaction.id, kind = %recorded.transaction.transaction_type, "agent recording");
        as_json(&serde_json::json!({
            "status": "recorded",
            "transaction": recorded.transaction,
            "account": recorded.account_name,
            "currency": recorded.currency,
            "asset": recorded.asset_reference,
            "zero_cost": recorded.zero_cost,
        }))
    }

    /// AGT-043 — the one account with this name, case ignored; the refusal names only what
    /// was sent (AGT-042).
    async fn account_named(&self, typed: &str) -> Result<String, Refusal> {
        let accounts = self
            .account_service
            .get_all_by_name()
            .await
            .map_err(|error| unreadable("list_holdings", &error))?;
        let mut matches = accounts
            .iter()
            .filter(|account| account.name.to_lowercase() == typed.to_lowercase());
        let found = matches.next().ok_or_else(|| Refusal {
            code: "AccountNotFound".to_string(),
            message: format!("no account named \"{}\"", typed.escape_debug()),
        })?;
        if matches.next().is_some() {
            return Err(Refusal {
                code: "AccountAmbiguous".to_string(),
                message: format!(
                    "more than one account is named \"{}\"",
                    typed.escape_debug()
                ),
            });
        }
        Ok(found.id.clone())
    }
}

/// AGT-048 — a figure as an agent sends it: a decimal with a dot and at most six decimals,
/// as text or as a JSON number.
fn figure(arguments: &Value, name: &str) -> Result<Option<i64>, Refusal> {
    let typed = match arguments.get(name) {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(_) => String::new(),
    };
    decimal_to_micro(&typed).map(Some).ok_or_else(|| Refusal {
        code: "InvalidFigure".to_string(),
        message: format!(
            "{name} is not a decimal with a dot and at most six decimals: \"{}\"",
            typed.escape_debug()
        ),
    })
}

fn amount(arguments: &Value, name: &str) -> Result<i64, Refusal> {
    figure(arguments, name)?.ok_or_else(|| Refusal {
        code: "MissingArgument".to_string(),
        message: format!("{name} is missing"),
    })
}

/// The account, the asset, the quantity and the date of a recording.
fn target(arguments: &Value) -> Result<Target, Refusal> {
    Ok(Target {
        account: text(arguments, "account")?.to_string(),
        asset: text(arguments, "asset")?.to_string(),
        date: optional_text(arguments, "date")?,
        quantity: amount(arguments, "quantity")?,
    })
}

/// A purchase or a sale: a unit price or an all-in total, never both (CLI-013).
fn trade(arguments: &Value) -> Result<Trade, Refusal> {
    let amount = match (figure(arguments, "price")?, figure(arguments, "total")?) {
        (Some(price), None) => TradeAmount::Price(price),
        (None, Some(total)) => TradeAmount::Total(total),
        _ => {
            return Err(Refusal {
                code: "InvalidArguments".to_string(),
                message: "give either price or total, not both".to_string(),
            })
        }
    };
    Ok(Trade {
        target: target(arguments)?,
        amount,
        fees: figure(arguments, "fees")?.unwrap_or(0),
        rate: figure(arguments, "rate")?.unwrap_or(1_000_000),
        note: note(arguments)?,
    })
}

/// The longest note an agent may attach to a recording, in characters.
const NOTE_LENGTH: usize = 500;

/// An optional text argument: absent or null is none; anything but text is refused, never
/// ignored — a date sent as a number must not become today.
fn optional_text(arguments: &Value, name: &str) -> Result<Option<String>, Refusal> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: format!("{name} is not text"),
        }),
    }
}

/// AGT-048 — the note of a recording, at most `NOTE_LENGTH` characters.
fn note(arguments: &Value) -> Result<Option<String>, Refusal> {
    let note = optional_text(arguments, "note")?;
    if note
        .as_ref()
        .is_some_and(|note| note.chars().count() > NOTE_LENGTH)
    {
        return Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: format!("note is longer than {NOTE_LENGTH} characters"),
        });
    }
    Ok(note)
}

fn text<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, Refusal> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| Refusal {
            code: "MissingArgument".to_string(),
            message: format!("{name} is missing"),
        })
}

fn as_json<T: serde::Serialize>(value: &T) -> Result<Value, Refusal> {
    serde_json::to_value(value).map_err(|error| {
        tracing::error!(target: BACKEND, err = ?error, "agent tool: result not serializable");
        portfolio_unreadable()
    })
}

fn unreadable(tool: &str, error: &dyn std::fmt::Debug) -> Refusal {
    tracing::error!(target: BACKEND, tool, err = ?error, "agent tool: read failed");
    portfolio_unreadable()
}

fn portfolio_unreadable() -> Refusal {
    Refusal {
        code: "DatabaseError".to_string(),
        message: "the portfolio could not be read".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const M: i64 = 1_000_000;

    fn code<T: std::fmt::Debug>(refused: Result<T, Refusal>) -> String {
        refused.expect_err("refused").code
    }

    // AGT-048 — a figure is a decimal with a dot, as text or as a JSON number; anything
    // else is refused, never guessed.
    #[test]
    fn agt_048_a_figure_is_a_decimal_as_text_or_as_a_number() {
        let arguments = json!({
            "text": "12.5", "number": 12.5, "whole": 3, "negative": -1, "nothing": null,
            "comma": "1,5", "exponent": 1e-7, "flag": true, "list": [1], "huge": "99999999999999999999",
        });
        assert_eq!(figure(&arguments, "text"), Ok(Some(12_500_000)));
        assert_eq!(figure(&arguments, "number"), Ok(Some(12_500_000)));
        assert_eq!(figure(&arguments, "whole"), Ok(Some(3 * M)));
        assert_eq!(figure(&arguments, "negative"), Ok(Some(-M)));
        assert_eq!(figure(&arguments, "nothing"), Ok(None));
        assert_eq!(figure(&arguments, "absent"), Ok(None));
        for refused in ["comma", "exponent", "flag", "list", "huge"] {
            assert_eq!(
                code(figure(&arguments, refused)),
                "InvalidFigure",
                "{refused}"
            );
        }
        assert_eq!(code(amount(&arguments, "absent")), "MissingArgument");
        assert_eq!(amount(&arguments, "whole"), Ok(3 * M));
    }

    // AGT-048 / AGT-044 — a purchase or a sale takes a unit price or a total, never both and never
    // neither; fees default to 0 and the rate to 1.
    #[test]
    fn agt_044_a_trade_takes_a_price_or_a_total_and_its_defaults() {
        let base = |extra: Value| {
            let mut arguments = json!({ "account": "PEA", "asset": "CW8", "quantity": "2" });
            for (name, value) in extra.as_object().expect("object") {
                arguments[name] = value.clone();
            }
            arguments
        };

        let priced = trade(&base(json!({ "price": "100.5" }))).expect("priced");
        assert_eq!(priced.amount, TradeAmount::Price(100_500_000));
        assert_eq!((priced.fees, priced.rate), (0, M));
        assert_eq!(priced.target.date, None);
        assert_eq!(priced.note, None);
        let total = trade(&base(json!({ "total": 201, "fees": "1.99", "rate": 1.1,
            "date": "2019-12-31", "note": "from a statement" })))
        .expect("total");
        assert_eq!(total.amount, TradeAmount::Total(201 * M));
        assert_eq!((total.fees, total.rate), (1_990_000, 1_100_000));
        assert_eq!(total.target.date.as_deref(), Some("2019-12-31"));
        assert_eq!(total.note.as_deref(), Some("from a statement"));

        assert_eq!(code(trade(&base(json!({})))), "InvalidArguments");
        assert_eq!(
            code(trade(&base(json!({ "price": 1, "total": 1 })))),
            "InvalidArguments"
        );
    }

    // AGT-048 — a date or a note that is not text is refused, never ignored: a date sent as
    // a number must not become today. A note is at most 500 characters.
    #[test]
    fn agt_048_a_date_or_a_note_that_is_not_text_is_refused() {
        let arguments = |extra: (&str, Value)| {
            let mut arguments =
                json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "price": 1 });
            arguments[extra.0] = extra.1;
            arguments
        };
        let refusal = trade(&arguments(("date", json!(20260105)))).expect_err("refused");
        assert_eq!(refusal.code, "InvalidArguments");
        assert_eq!(refusal.message, "date is not text");
        assert_eq!(
            code(trade(&arguments(("note", json!(["x"]))))),
            "InvalidArguments"
        );
        assert_eq!(
            code(trade(&arguments((
                "note",
                json!("x".repeat(NOTE_LENGTH + 1))
            )))),
            "InvalidArguments"
        );
        assert!(trade(&arguments(("note", json!("é".repeat(NOTE_LENGTH))))).is_ok());
        assert_eq!(
            code(target(
                &json!({ "account": " ", "asset": "CW8", "quantity": 1 })
            )),
            "MissingArgument"
        );
    }
}
