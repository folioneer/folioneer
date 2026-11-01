//! AGT-090 — an agent asks the owner to replace four transactions the owner typed — a
//! transfer recorded the four-step way — with the transfer they stand for. AGT-093 — four
//! that agents recorded it replaces without asking.

use std::sync::Arc;

use serde_json::Value;

use crate::context::account::{AgentRecording, TransactionType};
use crate::core::BACKEND;
use crate::use_cases::holding_transaction::TypedTransaction;
use crate::use_cases::transfer::{Replacement, ReplacementError};

use super::transfer::refusal_from_transfer;
use super::{
    as_json, figure, portfolio_unreadable, session_limit_reached, shown, AgentTools, Granted,
    Refusal,
};
use crate::agent_connection::connections::{ReplacedShown, ReplacementAnswer, TransferShown};

/// AGT-090 — how many transactions a replacement takes.
const FOUR: usize = 4;

impl AgentTools {
    /// AGT-090 — the session asks the owner to replace four transactions the owner typed
    /// with the transfer they stand for, and waits for the answer. What the window's
    /// transfer would refuse is refused first.
    pub(super) async fn request_transfer_replacement(
        &self,
        granted: &Granted,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        let ids = ids_of(arguments)?;
        let rate = figure(arguments, "rate")?;
        let mut typed = Vec::with_capacity(FOUR);
        for id in &ids {
            typed.push(
                self.recorder
                    .typed_by_owner(id)
                    .await
                    .map_err(|refusal| Refusal {
                        code: refusal.code,
                        message: refusal.message,
                    })?,
            );
        }
        let four = typed.iter().map(|t| t.transaction.clone()).collect();
        let asked = self
            .transfer
            .ask_replacement(four, rate)
            .await
            .map_err(|error| {
                let refusal = refusal_from_replacement(error);
                tracing::info!(target: BACKEND, session = granted.session_id, code = %refusal.code, "agent's replacement refused before asking the owner");
                refusal
            })?;
        let (transactions, transfer) = shown_of(&typed, &asked).ok_or_else(|| {
            tracing::error!(target: BACKEND, "request_transfer_replacement: a transaction of the four is not among those read");
            super::portfolio_unreadable()
        })?;
        tracing::info!(target: BACKEND, session = granted.session_id, "agent asks to replace four transactions the owner typed with a transfer");
        match self
            .connections
            .ask_replacement(granted, transactions, transfer, asked)
            .await
        {
            ReplacementAnswer::Replaced(transfer) => {
                as_json(&serde_json::json!({ "status": "replaced", "transfer": transfer }))
            }
            ReplacementAnswer::Refused => Err(Refusal {
                code: "ReplacementRefused".to_string(),
                message: "the owner did not replace these transactions".to_string(),
            }),
            ReplacementAnswer::NotReplaced { code, message } => Err(Refusal { code, message }),
        }
    }

    /// AGT-093 — the session replaces four transactions agents recorded with the transfer
    /// they stand for, asking nobody, the transfer's parts marked with this session.
    pub(super) async fn replace_with_transfer(
        &self,
        granted: &Granted,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        if !granted.may_record_one_more() {
            return Err(session_limit_reached());
        }
        let ids = ids_of(arguments)?;
        let rate = figure(arguments, "rate")?;
        let mut four = Vec::with_capacity(FOUR);
        for id in &ids {
            four.push(
                self.recorder
                    .recorded_by_an_agent(id)
                    .await
                    .map_err(|refusal| Refusal {
                        code: refusal.code,
                        message: refusal.message,
                    })?,
            );
        }
        let asked = self
            .transfer
            .ask_replacement(four, rate)
            .await
            .map_err(refusal_from_replacement)?;
        // AGT-047 — each part takes a place of the limit, as `record_transfer`'s do.
        if !granted.may_record(asked.preview.parts.len() as u32) {
            return Err(session_limit_reached());
        }
        let mark = AgentRecording {
            agent: granted.client.clone(),
            session: granted.key.clone(),
            session_started_at: granted.started_at.clone(),
        };
        let transfer = Arc::clone(&self.transfer);
        // AGT-049 — once started, the replacement runs to its end, whatever becomes of the
        // connection that asked.
        let parts = tokio::spawn(async move {
            let parts = transfer.replace(&asked, Some(&mark)).await?;
            let Some(parts) = parts else {
                return Ok(None);
            };
            let view = match parts.first().and_then(|part| part.transfer_id.clone()) {
                Some(transfer_id) => transfer.get_transfer(&transfer_id).await.ok(),
                None => None,
            };
            Ok(Some((parts.len(), view)))
        })
        .await
        .map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "agent replacement: the task ended early");
            portfolio_unreadable()
        })?
        .map_err(|error| refusal_from_transfer(&error))?;
        let Some((parts, view)) = parts else {
            return Err(Refusal {
                code: "TransactionChanged".to_string(),
                message: "the four transactions or the transfer changed since they were read: read them again".to_string(),
            });
        };
        for _ in 0..parts {
            granted.count_marked_transaction();
        }
        self.tell_recordings(granted).await;
        tracing::info!(target: BACKEND, session = granted.session_id, "agent replaced four recordings with a transfer");
        let Some(view) = view else {
            tracing::error!(target: BACKEND, "replace_with_transfer: the transfer recorded could not be read");
            return Err(Refusal {
                code: "DatabaseError".to_string(),
                message:
                    "the transfer was recorded, but could not be read: list_transactions shows it"
                        .to_string(),
            });
        };
        as_json(&serde_json::json!({ "status": "replaced", "transfer": view }))
    }
}

/// AGT-090 — what the checks of a replacement refuse, with its code.
fn refusal_from_replacement(error: ReplacementError) -> Refusal {
    match error {
        ReplacementError::NotATransfer(reason) => Refusal {
            code: "NotATransfer".to_string(),
            message: format!("{reason}: a transfer recorded the four-step way is a sale and a withdrawal in one account, a deposit and a purchase in another"),
        },
        ReplacementError::Refused(error) => refusal_from_transfer(&error),
    }
}

/// AGT-090 — the ids of the four transactions, as the agent sent them.
fn ids_of(arguments: &Value) -> Result<Vec<String>, Refusal> {
    let missing = || Refusal {
        code: "MissingArgument".to_string(),
        message:
            "transactions is missing: give the ids of the four, as list_transactions gives them"
                .to_string(),
    };
    let ids = arguments
        .get("transactions")
        .and_then(Value::as_array)
        .ok_or_else(missing)?
        .iter()
        .map(|id| id.as_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(missing)?;
    for id in &ids {
        super::bounded("transactions", id)?;
    }
    let distinct = ids.iter().collect::<std::collections::HashSet<_>>().len() == ids.len();
    if ids.len() != FOUR || !distinct {
        return Err(Refusal {
            code: "InvalidArguments".to_string(),
            message: "transactions names four transactions, each once".to_string(),
        });
    }
    Ok(ids)
}

/// AGT-091 — the four as the owner is shown them, in the order of the steps, and the
/// transfer recorded in their place; `None` when one of the steps is not among `typed`.
fn shown_of(
    typed: &[TypedTransaction],
    asked: &Replacement,
) -> Option<(Vec<ReplacedShown>, TransferShown)> {
    let steps = &asked.steps;
    let typed_of = |id: &str| typed.iter().find(|t| t.transaction.id == id);
    let sale = typed_of(&steps.sale.id)?;
    let purchase = typed_of(&steps.purchase.id)?;
    // AGT-092 — what each step moved: the sale takes its quantity out and brings its cash
    // in, the withdrawal takes cash out, the deposit brings it in, the purchase brings its
    // quantity in and takes its cash out.
    let transactions = [
        (&steps.sale, Some(-1), 1),
        (&steps.withdrawal, None, -1),
        (&steps.deposit, None, 1),
        (&steps.purchase, Some(1), -1),
    ]
    .into_iter()
    .map(|(step, holding, cash)| {
        typed_of(&step.id).map(|t| {
            let transaction = shown(t, &t.transaction);
            ReplacedShown {
                holding_moved: holding.map(|sign| sign * transaction.quantity),
                cash_moved: cash * transaction.amount,
                transaction,
            }
        })
    })
    .collect::<Option<Vec<_>>>()?;
    let between_currencies = sale.currency != purchase.currency;
    let converted = asked
        .preview
        .parts
        .iter()
        .find(|t| t.transaction_type == TransactionType::TransferIn)
        .map(|t| t.total_amount);
    let asset = sale.asset.as_ref()?;
    let transfer = TransferShown {
        from_account: sale.account_name.clone(),
        to_account: purchase.account_name.clone(),
        date: steps.sale.date.clone(),
        asset_name: asset.name.clone(),
        asset_reference: asset.reference.clone(),
        quantity_sent: steps.sale.quantity,
        quantity_received: steps.purchase.quantity,
        quantity_not_received: Some(steps.sale.quantity - steps.purchase.quantity)
            .filter(|not_received| *not_received > 0),
        cost: asked.cost(),
        from_currency: sale.currency.clone(),
        exchange_rate: between_currencies.then_some(asked.exchange_rate),
        converted_cost: converted.filter(|_| between_currencies),
        to_currency: purchase.currency.clone(),
    };
    Some((transactions, transfer))
}
