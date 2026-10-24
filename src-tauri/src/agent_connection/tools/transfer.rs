//! AGT-065A — a transfer an agent records: a holding moved from one account to another at
//! its cost (TRF), through the window's transfer and marked part by part.

use std::sync::Arc;

use serde_json::Value;

use crate::context::account::{AccountServiceContract, AgentRecording, Transaction};
use crate::core::BACKEND;
use crate::use_cases::holding_transaction::{code_of, refusal_from_account, NamedTarget};
use crate::use_cases::transfer::{TransferError, TransferFigures, TransferTask};

use super::{
    amount, as_json, figure, note, optional_text, portfolio_unreadable, session_limit_reached,
    text, today, AgentTools, Granted, Refusal,
};

/// A transfer as the agent sent it.
struct TypedTransfer {
    from: String,
    to: String,
    asset: String,
    date: Option<String>,
    quantity: i64,
    received: Option<i64>,
    rate: Option<i64>,
    note: Option<String>,
}

impl AgentTools {
    /// AGT-065A — records a transfer through the window's transfer (TRF-010 to TRF-022), each
    /// part marked with the agent and its session (AGT-045).
    pub(super) async fn record_transfer(
        &self,
        granted: &Granted,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        if !granted.may_record_one_more() {
            return Err(session_limit_reached());
        }
        let typed = transfer_of(arguments)?;
        let (sending, receiving, figures) = self.named(typed).await?;
        let mark = AgentRecording {
            agent: granted.client.clone(),
            session: granted.key.clone(),
            session_started_at: granted.started_at.clone(),
        };
        let transfer = Arc::clone(&self.transfer);
        let account_service = Arc::clone(&self.account_service);
        let (from_id, to_id, asset_id) = (
            sending.account_id.clone(),
            receiving.account_id.clone(),
            sending.asset_id.clone(),
        );
        // AGT-049 — once started, the transfer and its marks run to their end, whatever
        // becomes of the connection that asked.
        let outcome = tokio::spawn(async move {
            let parts = transfer
                .record_transfer(&from_id, &to_id, asset_id, figures)
                .await
                .map_err(|error| refusal_from_transfer(&error))?;
            marked(&*account_service, &parts, &mark).await?;
            Ok::<_, Refusal>(parts)
        })
        .await
        .map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "agent transfer: the task ended early");
            portfolio_unreadable()
        })?;
        let parts = outcome.inspect_err(|refusal| {
            // Parts that stayed without their mark were recorded all the same (AGT-047).
            if refusal.code == "RecordedNotMarked" {
                granted.count_recording();
            }
        })?;
        for _ in &parts {
            granted.count_marked_transaction();
        }
        self.tell_recordings(granted).await;
        tracing::info!(target: BACKEND, session = granted.session_id, parts = parts.len(), "agent recorded a transfer");
        as_json(&serde_json::json!({
            "status": "recorded",
            "transactions": parts,
            "from": sending.account_name,
            "from_currency": sending.currency,
            "to": receiving.account_name,
            "to_currency": receiving.currency,
            "asset": sending.asset_reference,
        }))
    }

    /// AGT-072 — what `record_transfer` would do, nothing written: the parts with no id and
    /// each account on the evening of the date without the transfer and with it.
    pub(super) async fn preview_transfer(&self, arguments: &Value) -> Result<Value, Refusal> {
        let typed = transfer_of(arguments)?;
        let (sending, receiving, figures) = self.named(typed).await?;
        let date = figures.date.clone();
        let previewed = self
            .transfer
            .preview_transfer(
                &sending.account_id,
                &receiving.account_id,
                sending.asset_id.clone(),
                figures,
            )
            .await
            .map_err(|error| refusal_from_transfer(&error))?;
        let mut parts =
            serde_json::to_value(&previewed.parts).map_err(|_| portfolio_unreadable())?;
        if let Some(parts) = parts.as_array_mut() {
            for part in parts {
                if let Some(id) = part.get_mut("id") {
                    *id = Value::Null;
                }
            }
        }
        let as_of = |transactions| self.recorder.account_as_of(transactions, &date);
        as_json(&serde_json::json!({
            "status": "previewed",
            "transactions": parts,
            "asset": sending.asset_reference,
            "date": date,
            "from": {
                "account": sending.account_name,
                "currency": sending.currency,
                "before": as_of(&previewed.sending_before).await,
                "after": as_of(&previewed.sending_after).await,
            },
            "to": {
                "account": receiving.account_name,
                "currency": receiving.currency,
                "before": as_of(&previewed.receiving_before).await,
                "after": as_of(&previewed.receiving_after).await,
            },
        }))
    }

    /// The two accounts and the asset found by name, and the figures the transfer takes:
    /// the quantity received the quantity sent when none is given (TRF-021).
    async fn named(
        &self,
        typed: TypedTransfer,
    ) -> Result<(NamedTarget, NamedTarget, TransferFigures), Refusal> {
        let (sending, receiving) = self
            .recorder
            .find_transfer_by_name(&typed.from, &typed.to, &typed.asset)
            .await
            .map_err(|refusal| Refusal {
                code: refusal.code,
                message: refusal.message,
            })?;
        let figures = TransferFigures {
            date: typed.date.unwrap_or_else(today),
            quantity_sent: typed.quantity,
            quantity_received: typed.received.unwrap_or(typed.quantity),
            exchange_rate: typed.rate,
            note: typed.note,
        };
        Ok((sending, receiving, figures))
    }
}

/// AGT-065A — a transfer read as `record_transfer` reads it.
fn transfer_of(arguments: &Value) -> Result<TypedTransfer, Refusal> {
    Ok(TypedTransfer {
        from: text(arguments, "from")?.to_string(),
        to: text(arguments, "to")?.to_string(),
        asset: text(arguments, "asset")?.to_string(),
        date: optional_text(arguments, "date")?,
        quantity: amount(arguments, "quantity")?,
        received: figure(arguments, "received")?,
        rate: figure(arguments, "rate")?,
        note: note(arguments)?,
    })
}

/// AGT-045 / AGT-049 — marks every part; when one cannot be, the transfer is undone.
async fn marked(
    account_service: &dyn AccountServiceContract,
    parts: &[Transaction],
    mark: &AgentRecording,
) -> Result<(), Refusal> {
    for part in parts {
        let Err(error) = account_service.mark_agent_recording(&part.id, mark).await else {
            continue;
        };
        tracing::error!(target: BACKEND, transaction = %part.id, err = ?error, "agent transfer: a part not marked, undoing the transfer");
        let Some(transfer_id) = part.transfer_id.as_deref() else {
            return Err(not_undone());
        };
        return match account_service.cancel_transfer(transfer_id).await {
            Ok(()) => Err(Refusal {
                code: "NotRecorded".to_string(),
                message: "the transfer could not be marked as an agent's, so nothing was recorded: try again".to_string(),
            }),
            Err(error) => {
                tracing::error!(target: BACKEND, transfer = %transfer_id, err = ?error, "agent transfer: not marked and not undone");
                Err(not_undone())
            }
        };
    }
    Ok(())
}

fn not_undone() -> Refusal {
    Refusal {
        code: "RecordedNotMarked".to_string(),
        message: "the transfer was recorded, but could not be marked as recorded by an agent: tell the owner".to_string(),
    }
}

/// AGT-065A — the transfer's refusals keep their codes (CLI-021).
fn refusal_from_transfer(error: &TransferError) -> Refusal {
    match error {
        TransferError::Account(error) => {
            let refusal = refusal_from_account(error);
            Refusal {
                code: refusal.code,
                message: refusal.message,
            }
        }
        TransferError::UseCase(task) => Refusal {
            code: code_of(task),
            message: match task {
                TransferTask::AssetNotFound => "the asset no longer exists".to_string(),
                TransferTask::ArchivedAsset => {
                    "the asset is archived: unarchive it to transfer it".to_string()
                }
                TransferTask::TransferOfCash => {
                    "cash is not transferred: record a withdrawal and a deposit".to_string()
                }
            },
        },
    }
}
