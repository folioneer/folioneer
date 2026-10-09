//! Running the tools an agent may call (AGT-040), in the running application: each is a
//! query the window already makes.

use std::sync::Arc;

use serde_json::Value;

use crate::context::account::{AccountError, AccountServiceContract};
use crate::context::asset::{Asset, AssetClass, AssetKind, AssetServiceContract};
use crate::context::currency::CurrencyService;
use crate::core::BACKEND;
use crate::use_cases::account_details::AccountDetailsUseCase;
use crate::use_cases::account_summary::AccountSummaryUseCase;

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

    /// AGT-040 — runs `tool` and returns its result, or why it is refused. A refusal names
    /// only what the agent sent (AGT-042).
    pub async fn call(&self, tool: &str, arguments: &Value) -> Result<Value, Refusal> {
        match tool {
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
