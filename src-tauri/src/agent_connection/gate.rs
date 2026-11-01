//! The owner's setting "Allow agents to connect" (AGT-022): off unless the owner switched
//! it on, kept on this computer across restarts, and the only thing that opens the channel.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::task::JoinHandle;

use crate::core::BACKEND;
use crate::use_cases::holding_transaction::{AsShown, HoldingTransactionUseCase, Refusal};
use crate::use_cases::transfer::TransferUseCase;

use super::channel;
use super::connections::{
    AgentConnections, ChangeAnswer, RemovalAnswer, ReplacementAnswer, TransactionShown,
};
use super::server::ToolRunner;
use super::tools::refusal_from_transfer;

/// Why the owner's answer to a removal, change or replacement request did nothing (AGT-081,
/// AGT-088, AGT-091).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerRefused {
    /// The request was already answered, or its session ended.
    Gone,
    /// The owner said yes and the transaction stays as it was: `reason` is the code the
    /// agent is given.
    NotDone {
        /// The code.
        reason: String,
    },
    /// The portfolio could not be read or written.
    Unreadable,
}

/// AGT-081 — the owner's answer to a removal request, over the connections and the
/// recorder: removing the transaction through the deletion of the window, or refusing. The
/// agent that asked is told what became of it.
pub async fn answer_removal(
    connections: &AgentConnections,
    recorder: &HoldingTransactionUseCase,
    request_id: u32,
    remove: bool,
) -> Result<(), AnswerRefused> {
    let answered = connections
        .take_removal(request_id)
        .ok_or(AnswerRefused::Gone)?;
    let transaction = answered.request.transaction.id.clone();
    if !remove {
        tracing::info!(target: BACKEND, session = answered.request.session, transaction = %transaction, "the owner refused a removal");
        answered.tell(RemovalAnswer::Refused);
        return Ok(());
    }
    match recorder
        .remove_typed_by_owner(&transaction, &as_shown(&answered.request.transaction))
        .await
    {
        Ok(_) => {
            tracing::info!(target: BACKEND, session = answered.request.session, transaction = %transaction, "the owner removed a transaction an agent asked to remove");
            answered.tell(RemovalAnswer::Removed);
            Ok(())
        }
        Err(refusal) => {
            tracing::warn!(target: BACKEND, session = answered.request.session, code = %refusal.code, "answer_removal_request: the deletion refused");
            let error = not_done(&refusal);
            answered.tell(RemovalAnswer::NotRemoved {
                code: refusal.code,
                message: refusal.message,
            });
            Err(error)
        }
    }
}

/// AGT-088 — the owner's answer to a change request, over the connections and the recorder:
/// changing the transaction through the correction of the window, or refusing. The agent
/// that asked is told what became of it.
pub async fn answer_change(
    connections: &AgentConnections,
    recorder: &HoldingTransactionUseCase,
    request_id: u32,
    change: bool,
) -> Result<(), AnswerRefused> {
    let answered = connections
        .take_change(request_id)
        .ok_or(AnswerRefused::Gone)?;
    let transaction = answered.request.transaction.id.clone();
    if !change {
        tracing::info!(target: BACKEND, session = answered.request.session, transaction = %transaction, "the owner refused a change");
        answered.tell(ChangeAnswer::Refused);
        return Ok(());
    }
    match recorder
        .change_typed_by_owner(
            &transaction,
            &as_shown(&answered.request.transaction),
            answered.held.clone(),
        )
        .await
    {
        Ok(changed) => {
            tracing::info!(target: BACKEND, session = answered.request.session, transaction = %transaction, "the owner changed a transaction an agent asked to change");
            answered.tell(ChangeAnswer::Changed(Box::new(changed)));
            Ok(())
        }
        Err(refusal) => {
            tracing::warn!(target: BACKEND, session = answered.request.session, code = %refusal.code, "answer_change_request: the correction refused");
            let error = not_done(&refusal);
            answered.tell(ChangeAnswer::NotChanged {
                code: refusal.code,
                message: refusal.message,
            });
            Err(error)
        }
    }
}

/// AGT-091 — the owner's answer to a replacement request, over the connections and the
/// transfer: removing the four transactions and recording the transfer in one write, or
/// refusing. The agent that asked is told what became of it.
pub async fn answer_replacement(
    connections: &AgentConnections,
    transfers: &TransferUseCase,
    request_id: u32,
    replace: bool,
) -> Result<(), AnswerRefused> {
    let answered = connections
        .take_replacement(request_id)
        .ok_or(AnswerRefused::Gone)?;
    let session = answered.request.session;
    if !replace {
        tracing::info!(target: BACKEND, session, "the owner refused a replacement");
        answered.tell(ReplacementAnswer::Refused);
        return Ok(());
    }
    let refusal = match transfers.replace(&answered.held, None).await {
        Ok(Some(parts)) => {
            tracing::info!(target: BACKEND, session, "the owner replaced four transactions with a transfer");
            let transfer_id = parts.first().and_then(|part| part.transfer_id.clone());
            let view = match transfer_id {
                Some(transfer_id) => transfers
                    .get_transfer(&transfer_id)
                    .await
                    .inspect_err(|err| tracing::error!(target: BACKEND, session, err = ?err, "answer_replacement_request: the transfer recorded could not be read"))
                    .ok(),
                None => {
                    tracing::error!(target: BACKEND, session, "answer_replacement_request: the transfer recorded has no parts");
                    None
                }
            };
            let Some(view) = view else {
                answered.tell(ReplacementAnswer::NotReplaced {
                    code: "DatabaseError".to_string(),
                    message: "the transfer was recorded, but could not be read: list_transactions shows it".to_string(),
                });
                return Ok(());
            };
            answered.tell(ReplacementAnswer::Replaced(Box::new(view)));
            return Ok(());
        }
        Ok(None) => Refusal {
            code: "TransactionChanged".to_string(),
            message: "the four transactions or the transfer changed since the owner was shown them: ask again".to_string(),
        },
        Err(error) => {
            let refusal = refusal_from_transfer(&error);
            Refusal {
                code: refusal.code,
                message: refusal.message,
            }
        }
    };
    tracing::warn!(target: BACKEND, session, code = %refusal.code, "answer_replacement_request: nothing replaced");
    let error = not_done(&refusal);
    answered.tell(ReplacementAnswer::NotReplaced {
        code: refusal.code,
        message: refusal.message,
    });
    Err(error)
}

/// What the owner was shown of `shown`, as an answer checks it still stands.
fn as_shown(shown: &TransactionShown) -> AsShown {
    AsShown {
        date: shown.date.clone(),
        kind: shown.kind,
        quantity: shown.quantity.clone(),
        amount: shown.amount,
        unit_price: shown.unit_price,
        fees: shown.fees,
        exchange_rate: shown.exchange_rate,
        note: shown.note.clone(),
    }
}

/// The window's refusal of a yes that did nothing.
fn not_done(refusal: &Refusal) -> AnswerRefused {
    if refusal.code == "DatabaseError" {
        AnswerRefused::Unreadable
    } else {
        AnswerRefused::NotDone {
            reason: refusal.code.clone(),
        }
    }
}

/// AGT-038 — how many bridges may be connected at once.
pub const MAX_CONNECTIONS: usize = 8;

/// The file whose presence in the data folder means the owner allows agents to connect.
const ALLOWED_FILE: &str = "agents-allowed";

/// Why the setting could not be changed.
#[derive(Debug, PartialEq, Eq)]
pub enum GateError {
    /// This system has no channel (AGT-023).
    Unavailable,
    /// The setting or the channel could not be written (AGT-024).
    Failed,
}

/// Opens and closes the channel as the owner's setting says.
pub struct AgentGate {
    data_dir: PathBuf,
    connections: Arc<AgentConnections>,
    // Read where a channel exists (AGT-023).
    #[cfg_attr(not(any(unix, windows)), allow(dead_code))]
    tools: Arc<dyn ToolRunner>,
    listening: Mutex<Option<JoinHandle<()>>>,
}

impl AgentGate {
    /// A closed gate over the data folder; `open_if_allowed` opens it at start.
    pub fn new(
        data_dir: PathBuf,
        connections: Arc<AgentConnections>,
        tools: Arc<dyn ToolRunner>,
    ) -> Self {
        Self {
            data_dir,
            connections,
            tools,
            listening: Mutex::new(None),
        }
    }

    fn allowed_file(&self) -> PathBuf {
        self.data_dir.join(ALLOWED_FILE)
    }

    /// Whether the owner allows agents to connect.
    pub fn is_allowed(&self) -> bool {
        channel::AVAILABLE && self.allowed_file().exists()
    }

    /// The connections this gate lets in.
    pub fn connections(&self) -> &Arc<AgentConnections> {
        &self.connections
    }

    /// AGT-022 — at start: opens the channel when the setting is on. Must be called from
    /// within the application's async runtime.
    pub fn open_if_allowed(&self) {
        if self.is_allowed() {
            if let Err(error) = self.open() {
                tracing::warn!(target: BACKEND, err = ?error, "agent channel not opened at start");
            }
        }
    }

    /// AGT-022 — the owner switches the setting: on opens the channel; off closes it,
    /// refuses who waits and ends who is connected (AGT-035). Must be called from within
    /// the application's async runtime.
    pub fn allow(&self, allowed: bool) -> Result<(), GateError> {
        if !allowed {
            let removed = match std::fs::remove_file(self.allowed_file()) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            };
            self.close();
            self.connections.notify();
            return removed.map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "agent setting not switched off");
                GateError::Failed
            });
        }
        if !channel::AVAILABLE {
            return Err(GateError::Unavailable);
        }
        self.open()?;
        std::fs::write(self.allowed_file(), b"").map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "agent setting not switched on");
            self.close();
            GateError::Failed
        })?;
        self.connections.notify();
        Ok(())
    }

    /// Closes the channel and ends every connection, served or not yet asking; the setting
    /// is left as it is.
    pub fn close(&self) {
        let listening = self
            .listening
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        self.connections.end_all();
        // Only the application that opened the channel removes it: another one running
        // over the same folder keeps its own.
        if let Some(listening) = listening {
            listening.abort();
            channel::close(&self.data_dir);
        }
    }

    #[cfg(any(unix, windows))]
    fn open(&self) -> Result<(), GateError> {
        let mut listening = self
            .listening
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if listening.is_some() {
            return Ok(());
        }
        let listener = channel::listen(&self.data_dir).map_err(|error| {
            tracing::error!(target: BACKEND, err = ?error, "agent channel not opened");
            GateError::Failed
        })?;
        self.connections.admit();
        *listening = Some(tokio::spawn(accept(
            listener,
            self.data_dir.clone(),
            Arc::clone(&self.connections),
            Arc::clone(&self.tools),
        )));
        tracing::info!(target: BACKEND, "agent channel opened");
        Ok(())
    }

    #[cfg(not(any(unix, windows)))]
    fn open(&self) -> Result<(), GateError> {
        Err(GateError::Unavailable)
    }
}

impl Drop for AgentGate {
    fn drop(&mut self) {
        self.close();
    }
}

/// Serves each bridge that connects, as long as it runs as the owner's user (AGT-021) and
/// no more than `MAX_CONNECTIONS` are served (AGT-038). The connections live and end with
/// this task: closing the gate drops them all (AGT-035).
#[cfg(any(unix, windows))]
async fn accept(
    mut listener: channel::Listener,
    data_dir: PathBuf,
    connections: Arc<AgentConnections>,
    tools: Arc<dyn ToolRunner>,
) {
    let mut served = tokio::task::JoinSet::new();
    let mut failing = false;
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            Some(_) = served.join_next(), if !served.is_empty() => continue,
        };
        let stream = match accepted {
            Ok(stream) => {
                failing = false;
                stream
            }
            Err(error) => {
                if !failing {
                    tracing::warn!(target: BACKEND, err = ?error, "agent channel: accept failed");
                }
                failing = true;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };
        if !channel::same_user(&stream, &data_dir) {
            tracing::warn!(target: BACKEND, "agent channel: a program of another user was turned away");
            continue;
        }
        if served.len() >= MAX_CONNECTIONS {
            tracing::warn!(target: BACKEND, "agent channel: too many connections, one was turned away");
            continue;
        }
        served.spawn(super::server::serve_connection(
            stream,
            Arc::clone(&connections),
            Arc::clone(&tools),
        ));
    }
}
