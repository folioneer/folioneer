//! AGT-090 — an agent asks the owner to replace four transactions the owner typed — a
//! transfer recorded the four-step way — with the transfer they stand for.

use serde_json::Value;

use crate::context::account::TransactionType;
use crate::core::BACKEND;
use crate::use_cases::holding_transaction::TypedTransaction;
use crate::use_cases::transfer::{Replacement, ReplacementError};

use super::transfer::refusal_from_transfer;
use super::{as_json, figure, shown, AgentTools, Granted, Refusal};
use crate::agent_connection::connections::{ReplacementAnswer, TransactionShown, TransferShown};

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
                let refusal = match error {
                    ReplacementError::NotATransfer(reason) => Refusal {
                        code: "NotATransfer".to_string(),
                        message: format!("{reason}: a transfer recorded the four-step way is a sale and a withdrawal in one account, a deposit and a purchase in another"),
                    },
                    ReplacementError::Refused(error) => refusal_from_transfer(&error),
                };
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
) -> Option<(Vec<TransactionShown>, TransferShown)> {
    let steps = &asked.steps;
    let typed_of = |id: &str| typed.iter().find(|t| t.transaction.id == id);
    let sale = typed_of(&steps.sale.id)?;
    let purchase = typed_of(&steps.purchase.id)?;
    let transactions = [
        &steps.sale,
        &steps.withdrawal,
        &steps.deposit,
        &steps.purchase,
    ]
    .into_iter()
    .map(|step| typed_of(&step.id).map(|t| shown(t, &t.transaction)))
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
        cost: asked.cost(),
        from_currency: sale.currency.clone(),
        exchange_rate: between_currencies.then_some(asked.exchange_rate),
        converted_cost: converted.filter(|_| between_currencies),
        to_currency: purchase.currency.clone(),
    };
    Some((transactions, transfer))
}
