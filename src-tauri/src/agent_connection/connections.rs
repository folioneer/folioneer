//! Who is connected and who asks to be (AGT-030 to AGT-035): the owner's answers and the
//! sessions they open, held in memory by the running application and gone when it closes.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use specta::Type;
use tokio::sync::{oneshot, Notify};

use crate::context::account::{Transaction, TransactionType};
use crate::use_cases::holding_transaction::CorrectionFields;

/// An agent client waiting for the owner's answer (AGT-030).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AgentConnectionRequest {
    /// Identifies the request in the owner's answer.
    pub id: u32,
    /// The name the agent client gave for itself.
    pub client: String,
    /// When it asked, as an RFC 3339 timestamp in this computer's time.
    pub asked_at: String,
}

/// A connected agent client (AGT-033), with what its session did (AGT-052).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AgentSession {
    /// Identifies the session when the owner disconnects it.
    pub id: u32,
    /// The name the agent client gave for itself.
    pub client: String,
    /// How many tool calls it made.
    pub calls: u32,
    /// When the owner allowed it, as an RFC 3339 timestamp in this computer's time.
    pub started_at: String,
    /// How many of its calls read the portfolio and were answered.
    pub reads: u32,
    /// How many transactions it recorded that still exist: its own to remove.
    pub recordings: u32,
    /// How many transactions it recorded and marked, whatever became of them since: a count
    /// that only rises, by which the window tells a new recording (AGT-084).
    pub transactions_recorded: u32,
    /// The one it recorded last among them.
    pub last_recording: Option<LastRecording>,
    /// How many assets it added (AGT-064).
    pub assets_added: u32,
    /// The one it added last.
    pub last_asset: Option<LastAsset>,
}

/// The last asset a session added, as the window names it (AGT-064).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct LastAsset {
    /// The asset's name.
    pub name: String,
    /// The asset's reference.
    pub reference: String,
}

/// The last transaction a session recorded, as the window names it (AGT-052).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct LastRecording {
    /// What kind of transaction.
    pub kind: TransactionType,
    /// The asset's reference.
    pub asset: String,
    /// The transaction's date.
    pub date: String,
}

/// A transaction the owner typed, as the owner is shown it before removing or changing it
/// (AGT-081, AGT-088).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct TransactionShown {
    /// The transaction's id.
    pub id: String,
    /// Its date.
    pub date: String,
    /// The name of its account.
    pub account: String,
    /// The account's currency, which its amount is in.
    pub currency: String,
    /// What kind of transaction.
    pub kind: TransactionType,
    /// Its asset's name; `None` for a transaction on the account's cash.
    pub asset_name: Option<String>,
    /// Its asset's reference; `None` for a transaction on the account's cash.
    pub asset_reference: Option<String>,
    /// Its quantity, in micro-units.
    pub quantity: i64,
    /// Its amount in the account's currency, in micro-units.
    pub amount: i64,
    /// Its unit price, in micro-units.
    pub unit_price: i64,
    /// Its fees, in micro-units.
    pub fees: i64,
    /// Its exchange rate, in micro-units.
    pub exchange_rate: i64,
    /// Its note.
    pub note: Option<String>,
}

/// A value of a transaction the owner is shown, as a change request names what it moves
/// (AGT-088).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ShownValue {
    /// Its date.
    Date,
    /// Its quantity.
    Quantity,
    /// Its amount.
    Amount,
    /// Its unit price.
    UnitPrice,
    /// Its fees.
    Fees,
    /// Its exchange rate.
    ExchangeRate,
    /// Its note.
    Note,
}

impl TransactionShown {
    /// AGT-088 — the values `becomes` holds that differ from these.
    pub fn moved_to(&self, becomes: &TransactionShown) -> Vec<ShownValue> {
        [
            (ShownValue::Date, self.date != becomes.date),
            (ShownValue::Quantity, self.quantity != becomes.quantity),
            (ShownValue::Amount, self.amount != becomes.amount),
            (ShownValue::UnitPrice, self.unit_price != becomes.unit_price),
            (ShownValue::Fees, self.fees != becomes.fees),
            (
                ShownValue::ExchangeRate,
                self.exchange_rate != becomes.exchange_rate,
            ),
            (ShownValue::Note, self.note != becomes.note),
        ]
        .into_iter()
        .filter_map(|(value, moves)| moves.then_some(value))
        .collect()
    }
}

/// An agent asking the owner to remove a transaction the owner typed (AGT-080).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct RemovalRequest {
    /// Identifies the request in the owner's answer.
    pub id: u32,
    /// The session that asks.
    pub session: u32,
    /// The name of the agent that asks.
    pub client: String,
    /// When it asked, as an RFC 3339 timestamp in this computer's time.
    pub asked_at: String,
    /// The transaction, as it stood when the agent asked.
    pub transaction: TransactionShown,
}

/// What became of a removal request, as the agent that asked is told (AGT-080).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemovalAnswer {
    /// The owner removed the transaction.
    Removed,
    /// The owner refused, or the session ended before an answer.
    Refused,
    /// The owner removed it and the deletion refused: its code and reason.
    NotRemoved {
        /// The deletion's code.
        code: String,
        /// The deletion's reason.
        message: String,
    },
}

/// An agent asking the owner to change a transaction the owner typed (AGT-087).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ChangeRequest {
    /// Identifies the request in the owner's answer; removal and change requests share one
    /// order (AGT-088).
    pub id: u32,
    /// The session that asks.
    pub session: u32,
    /// The name of the agent that asks.
    pub client: String,
    /// When it asked, as an RFC 3339 timestamp in this computer's time.
    pub asked_at: String,
    /// The transaction, as it stood when the agent asked.
    pub transaction: TransactionShown,
    /// The transaction as the change would make it.
    pub becomes: TransactionShown,
    /// The values the change moves, in the order they are shown (AGT-088).
    pub moved: Vec<ShownValue>,
}

/// What became of a change request, as the agent that asked is told (AGT-087).
#[derive(Debug, Clone)]
pub enum ChangeAnswer {
    /// The owner changed the transaction: as it now stands.
    Changed(Box<Transaction>),
    /// The owner refused, or the session ended before an answer.
    Refused,
    /// The owner said yes and the correction refused: its code and reason.
    NotChanged {
        /// The correction's code.
        code: String,
        /// The correction's reason.
        message: String,
    },
}

/// A request waiting for the owner's answer, with what performing it needs and the call that
/// waits on it: whoever takes it to perform the owner's answer tells the agent what became
/// of it.
pub struct Waiting<R, A, H = ()> {
    /// The request.
    pub request: R,
    /// What performing the owner's yes needs beyond what the window is shown.
    pub held: H,
    answer: oneshot::Sender<A>,
}

impl<R, A, H> Waiting<R, A, H> {
    /// Tells the agent that asked what became of its request; it may be gone.
    pub fn tell(self, answer: A) {
        let _ = self.answer.send(answer);
    }
}

/// A removal request waiting for the owner's answer (AGT-080).
pub type WaitingRemoval = Waiting<RemovalRequest, RemovalAnswer>;

/// A change request waiting for the owner's answer, with what its correction is given
/// (AGT-087).
pub type WaitingChange = Waiting<ChangeRequest, ChangeAnswer, CorrectionFields>;

/// What a session did, shared by the connection that serves it and by what the window is
/// told of it.
#[derive(Debug, Default)]
struct Facts {
    reads: AtomicU32,
    /// How many transactions it recorded, whatever became of them (AGT-084).
    recorded: AtomicU32,
    /// How many of its recordings still exist, and the last of them: read from the
    /// portfolio after each change, so the two always agree.
    recordings: Mutex<(u32, Option<LastRecording>)>,
    /// How many assets it added, and the last of them (AGT-064): counted as they are
    /// added, and kept until the session ends.
    assets: Mutex<(u32, Option<LastAsset>)>,
}

impl Facts {
    fn assets(&self) -> MutexGuard<'_, (u32, Option<LastAsset>)> {
        self.assets.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn recordings(&self) -> MutexGuard<'_, (u32, Option<LastRecording>)> {
        self.recordings
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// A session the owner allowed, as the connection serving it holds it.
#[derive(Debug, Clone)]
pub struct Granted {
    /// The session.
    pub session_id: u32,
    /// The name the agent gave for itself, cleaned (AGT-037).
    pub client: String,
    /// Identifies the session in what it records (AGT-045): unlike `session_id`, it is
    /// never used again by another run of the application.
    pub key: String,
    /// When the owner allowed it, as an RFC 3339 timestamp in this computer's time.
    pub started_at: String,
    /// How many transactions it recorded (AGT-047).
    recorded: Arc<AtomicU32>,
    /// What it did, as the window is told (AGT-052).
    facts: Arc<Facts>,
    /// Notified when the owner disconnects the session (AGT-034).
    pub ended: Arc<Notify>,
}

/// AGT-047 — how many transactions one session may record.
pub const MAX_RECORDINGS: u32 = 500;

impl Granted {
    /// AGT-047 — whether the session is still under its limit of recordings.
    pub fn may_record_one_more(&self) -> bool {
        self.recorded.load(Ordering::SeqCst) < MAX_RECORDINGS
    }

    /// Counts one transaction the session recorded.
    pub fn count_recording(&self) {
        self.recorded.fetch_add(1, Ordering::SeqCst);
    }

    /// AGT-047 / AGT-084 — the session recorded and marked one more transaction: one more
    /// place of its limit taken, and one more recording for the window to say.
    pub fn count_marked_transaction(&self) {
        self.count_recording();
        self.facts.recorded.fetch_add(1, Ordering::SeqCst);
    }

    /// AGT-052 — one call that read the portfolio was answered.
    pub fn count_read(&self) {
        self.facts.reads.fetch_add(1, Ordering::SeqCst);
    }

    /// AGT-064 — the session added an asset: one more, and the last of them.
    pub fn count_asset(&self, name: String, reference: String) {
        let mut assets = self.facts.assets();
        *assets = (assets.0 + 1, Some(LastAsset { name, reference }));
    }

    /// AGT-052 — what the session recorded and that still exists, as the portfolio has it
    /// now: how many, and the last of them.
    pub fn recordings_are(&self, count: u32, last: Option<LastRecording>) {
        *self.facts.recordings() = (count, last);
    }
}

struct Pending {
    request: AgentConnectionRequest,
    answer: oneshot::Sender<bool>,
}

struct Open {
    session: AgentSession,
    ended: Arc<Notify>,
    /// Identifies the session in what it recorded (AGT-045).
    key: String,
    facts: Arc<Facts>,
}

impl Open {
    /// The session with what it did so far.
    fn told(&self) -> AgentSession {
        let (recordings, last_recording) = self.facts.recordings().clone();
        let (assets_added, last_asset) = self.facts.assets().clone();
        AgentSession {
            reads: self.facts.reads.load(Ordering::SeqCst),
            transactions_recorded: self.facts.recorded.load(Ordering::SeqCst),
            recordings,
            last_recording,
            assets_added,
            last_asset,
            ..self.session.clone()
        }
    }
}

/// AGT-038 — how many agents may wait for the owner's answer at once.
pub const MAX_REQUESTS: usize = 4;

#[derive(Default)]
struct State {
    /// Whether the owner's setting lets agents in; nobody is asked for, and no session
    /// opens, while it does not (AGT-035).
    admitting: bool,
    last_id: u32,
    pending: Vec<Pending>,
    open: Vec<Open>,
    removals: Vec<WaitingRemoval>,
    changes: Vec<WaitingChange>,
}

/// The requests waiting and the sessions open. `changed` is called after every change, so
/// the window can be told (AGT-036).
pub struct AgentConnections {
    state: Mutex<State>,
    changed: Box<dyn Fn() + Send + Sync>,
}

/// Withdraws a request whose asker went away before the owner answered.
struct Withdrawal<'a> {
    connections: &'a AgentConnections,
    request_id: u32,
}

impl Drop for Withdrawal<'_> {
    fn drop(&mut self) {
        let withdrawn = {
            let mut state = self.connections.state();
            let before = state.pending.len();
            state
                .pending
                .retain(|pending| pending.request.id != self.request_id);
            state.pending.len() != before
        };
        if withdrawn {
            (self.connections.changed)();
        }
    }
}

/// Withdraws a removal or change request whose call went away before the owner answered.
struct RequestWithdrawal<'a> {
    connections: &'a AgentConnections,
    request_id: u32,
    /// Removes the request from those waiting; `true` when it was there.
    withdraw: fn(&mut State, u32) -> bool,
}

impl Drop for RequestWithdrawal<'_> {
    fn drop(&mut self) {
        let withdrawn = (self.withdraw)(&mut self.connections.state(), self.request_id);
        if withdrawn {
            (self.connections.changed)();
        }
    }
}

impl State {
    fn withdraw_removal(&mut self, request_id: u32) -> bool {
        let before = self.removals.len();
        self.removals
            .retain(|pending| pending.request.id != request_id);
        self.removals.len() != before
    }

    fn withdraw_change(&mut self, request_id: u32) -> bool {
        let before = self.changes.len();
        self.changes
            .retain(|pending| pending.request.id != request_id);
        self.changes.len() != before
    }

    fn is_open(&self, session_id: u32) -> bool {
        self.open.iter().any(|open| open.session.id == session_id)
    }
}

impl AgentConnections {
    /// No request, no session, and nobody admitted until `admit`; `changed` is called after
    /// every change.
    pub fn new(changed: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            state: Mutex::new(State::default()),
            changed: Box::new(changed),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Tells the window that something it shows changed (AGT-036).
    pub fn notify(&self) {
        (self.changed)();
    }

    /// AGT-022 — the owner's setting lets agents ask to connect.
    pub fn admit(&self) {
        self.state().admitting = true;
    }

    /// AGT-030 — asks the owner whether `client` may connect and waits for the answer:
    /// the session when allowed, `None` when refused, when agents are not admitted, or
    /// when too many already wait (AGT-038). A caller that stops waiting withdraws its
    /// request.
    pub async fn ask(&self, client: &str) -> Option<Granted> {
        let (answer, answered) = oneshot::channel();
        let request_id = {
            let mut state = self.state();
            if !state.admitting || state.pending.len() >= MAX_REQUESTS {
                return None;
            }
            state.last_id = state.last_id.wrapping_add(1);
            let request = AgentConnectionRequest {
                id: state.last_id,
                client: client.to_string(),
                asked_at: chrono::Local::now().to_rfc3339(),
            };
            state.pending.push(Pending { request, answer });
            state.last_id
        };
        (self.changed)();
        let withdrawal = Withdrawal {
            connections: self,
            request_id,
        };
        let allowed = answered.await.unwrap_or(false);
        drop(withdrawal);
        if !allowed {
            return None;
        }
        let ended = Arc::new(Notify::new());
        let key = uuid::Uuid::new_v4().to_string();
        let started_at = chrono::Local::now().to_rfc3339();
        let facts = Arc::new(Facts::default());
        {
            let mut state = self.state();
            // AGT-035 — the setting went off between the answer and here: no session.
            if !state.admitting {
                return None;
            }
            state.open.push(Open {
                session: AgentSession {
                    id: request_id,
                    client: client.to_string(),
                    calls: 0,
                    started_at: started_at.clone(),
                    reads: 0,
                    recordings: 0,
                    transactions_recorded: 0,
                    last_recording: None,
                    assets_added: 0,
                    last_asset: None,
                },
                ended: Arc::clone(&ended),
                key: key.clone(),
                facts: Arc::clone(&facts),
            });
        }
        (self.changed)();
        Some(Granted {
            session_id: request_id,
            client: client.to_string(),
            key,
            started_at,
            recorded: Arc::new(AtomicU32::new(0)),
            facts,
            ended,
        })
    }

    /// AGT-032 — the owner's answer to a request; `false` when no such request waits.
    pub fn answer(&self, request_id: u32, allow: bool) -> bool {
        let pending = {
            let mut state = self.state();
            let position = state
                .pending
                .iter()
                .position(|pending| pending.request.id == request_id);
            position.map(|position| state.pending.remove(position))
        };
        match pending {
            None => false,
            Some(pending) => {
                // The asker may be gone: its request is withdrawn either way.
                let _ = pending.answer.send(allow);
                (self.changed)();
                true
            }
        }
    }

    /// AGT-033 — counts one tool call of a session.
    pub fn count_call(&self, session_id: u32) {
        let counted = {
            let mut state = self.state();
            state
                .open
                .iter_mut()
                .find(|open| open.session.id == session_id)
                .map(|open| open.session.calls = open.session.calls.saturating_add(1))
                .is_some()
        };
        if counted {
            (self.changed)();
        }
    }

    /// AGT-034 — ends a session, on the owner's word or because its connection closed;
    /// `false` when no such session is open. The connection serving it is told.
    pub fn end(&self, session_id: u32) -> bool {
        let ended = {
            let mut state = self.state();
            let position = state
                .open
                .iter()
                .position(|open| open.session.id == session_id);
            let ended = position.map(|position| state.open.remove(position));
            // AGT-082 — its removal and change requests are withdrawn: their calls are refused.
            if ended.is_some() {
                state
                    .removals
                    .retain(|pending| pending.request.session != session_id);
                state
                    .changes
                    .retain(|pending| pending.request.session != session_id);
            }
            ended
        };
        match ended {
            None => false,
            Some(open) => {
                open.ended.notify_one();
                (self.changed)();
                true
            }
        }
    }

    /// AGT-035 — stops admitting agents, refuses every request and ends every session.
    pub fn end_all(&self) {
        let (pending, open, removals, changes) = {
            let mut state = self.state();
            state.admitting = false;
            (
                std::mem::take(&mut state.pending),
                std::mem::take(&mut state.open),
                std::mem::take(&mut state.removals),
                std::mem::take(&mut state.changes),
            )
        };
        if pending.is_empty() && open.is_empty() && removals.is_empty() && changes.is_empty() {
            return;
        }
        for pending in pending {
            let _ = pending.answer.send(false);
        }
        for open in open {
            open.ended.notify_one();
        }
        // AGT-082 — the removal and change requests' calls are refused as their answers drop.
        drop(removals);
        drop(changes);
        (self.changed)();
    }

    /// AGT-080 — the session `granted` asks the owner to remove the transaction `shown`
    /// and waits for the answer: `Refused` when the session is no longer open, or ends
    /// before an answer. A caller that stops waiting withdraws its request.
    ///
    /// No limit is kept here: a session's connection serves its calls one after the other
    /// (`server.rs`), so at most one request waits per session, and sessions are limited
    /// (AGT-038).
    pub async fn ask_removal(&self, granted: &Granted, shown: TransactionShown) -> RemovalAnswer {
        let (answer, answered) = oneshot::channel();
        let request_id = {
            let mut state = self.state();
            if !state.is_open(granted.session_id) {
                return RemovalAnswer::Refused;
            }
            state.last_id = state.last_id.wrapping_add(1);
            let request = RemovalRequest {
                id: state.last_id,
                session: granted.session_id,
                client: granted.client.clone(),
                asked_at: chrono::Local::now().to_rfc3339(),
                transaction: shown,
            };
            state.removals.push(WaitingRemoval {
                request,
                held: (),
                answer,
            });
            state.last_id
        };
        (self.changed)();
        let _withdrawal = RequestWithdrawal {
            connections: self,
            request_id,
            withdraw: State::withdraw_removal,
        };
        answered.await.unwrap_or(RemovalAnswer::Refused)
    }

    /// AGT-087 — the session `granted` asks the owner to change the transaction `shown` into
    /// `becomes`, and waits for the answer: `Refused` when the session is no longer open, or
    /// ends before an answer. `fields` is what the correction is given on a yes. A caller
    /// that stops waiting withdraws its request.
    pub async fn ask_change(
        &self,
        granted: &Granted,
        shown: TransactionShown,
        becomes: TransactionShown,
        fields: CorrectionFields,
    ) -> ChangeAnswer {
        let (answer, answered) = oneshot::channel();
        let request_id = {
            let mut state = self.state();
            if !state.is_open(granted.session_id) {
                return ChangeAnswer::Refused;
            }
            state.last_id = state.last_id.wrapping_add(1);
            let request = ChangeRequest {
                id: state.last_id,
                session: granted.session_id,
                client: granted.client.clone(),
                asked_at: chrono::Local::now().to_rfc3339(),
                moved: shown.moved_to(&becomes),
                transaction: shown,
                becomes,
            };
            state.changes.push(WaitingChange {
                request,
                held: fields,
                answer,
            });
            state.last_id
        };
        (self.changed)();
        let _withdrawal = RequestWithdrawal {
            connections: self,
            request_id,
            withdraw: State::withdraw_change,
        };
        answered.await.unwrap_or(ChangeAnswer::Refused)
    }

    /// AGT-088 — takes the change request the owner answered from those waiting; `None`
    /// when no such request waits. The window is told it is gone.
    pub fn take_change(&self, request_id: u32) -> Option<WaitingChange> {
        let taken = {
            let mut state = self.state();
            let position = state
                .changes
                .iter()
                .position(|pending| pending.request.id == request_id);
            position.map(|position| state.changes.remove(position))
        }?;
        (self.changed)();
        Some(taken)
    }

    /// The change requests waiting, oldest first (AGT-088).
    pub fn changes(&self) -> Vec<ChangeRequest> {
        self.state()
            .changes
            .iter()
            .map(|pending| pending.request.clone())
            .collect()
    }

    /// AGT-081 — takes the removal request the owner answered from those waiting; `None`
    /// when no such request waits. The window is told it is gone.
    pub fn take_removal(&self, request_id: u32) -> Option<WaitingRemoval> {
        let taken = {
            let mut state = self.state();
            let position = state
                .removals
                .iter()
                .position(|pending| pending.request.id == request_id);
            position.map(|position| state.removals.remove(position))
        }?;
        (self.changed)();
        Some(taken)
    }

    /// The removal requests waiting, oldest first (AGT-081).
    pub fn removals(&self) -> Vec<RemovalRequest> {
        self.state()
            .removals
            .iter()
            .map(|pending| pending.request.clone())
            .collect()
    }

    /// The requests waiting, oldest first.
    pub fn requests(&self) -> Vec<AgentConnectionRequest> {
        self.state()
            .pending
            .iter()
            .map(|pending| pending.request.clone())
            .collect()
    }

    /// The sessions open, oldest first, each with what it did so far (AGT-052).
    pub fn sessions(&self) -> Vec<AgentSession> {
        self.state().open.iter().map(Open::told).collect()
    }

    /// AGT-053 — what identifies an open session in what it recorded; `None` once it ended.
    pub fn key_of(&self, session_id: u32) -> Option<String> {
        self.state()
            .open
            .iter()
            .find(|open| open.session.id == session_id)
            .map(|open| open.key.clone())
    }

    /// AGT-053 — the owner removed what an open session recorded: what is left of its
    /// recordings, as the portfolio has it now. The window is told.
    pub fn recordings_are(&self, session_id: u32, count: u32, last: Option<LastRecording>) {
        let found = {
            let state = self.state();
            state
                .open
                .iter()
                .find(|open| open.session.id == session_id)
                .map(|open| *open.facts.recordings() = (count, last))
                .is_some()
        };
        if found {
            (self.changed)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn counted() -> (Arc<AgentConnections>, Arc<AtomicU32>) {
        let changes = Arc::new(AtomicU32::new(0));
        let seen = Arc::clone(&changes);
        let connections = Arc::new(AgentConnections::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        connections.admit();
        (connections, changes)
    }

    async fn waiting(connections: &AgentConnections) -> AgentConnectionRequest {
        for _ in 0..200 {
            if let Some(request) = connections.requests().into_iter().next() {
                return request;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("no request arrived");
    }

    // AGT-030 / AGT-032 — a connection waits for the owner's answer: allowed, it opens a
    // session under the client's name; refused, it opens none. Each step tells the window.
    #[tokio::test]
    async fn agt_030_a_connection_waits_for_the_owner_and_a_refusal_opens_nothing() {
        let (connections, changes) = counted();

        let asking = Arc::clone(&connections);
        let asked = tokio::spawn(async move { asking.ask("claude-code").await });
        let request = waiting(&connections).await;
        assert_eq!(request.client, "claude-code");
        assert!(connections.sessions().is_empty());
        assert!(!asked.is_finished());

        assert!(connections.answer(request.id, true));
        let granted = asked.await.expect("asked").expect("granted");
        assert_eq!(
            connections.sessions(),
            vec![AgentSession {
                id: granted.session_id,
                client: "claude-code".to_string(),
                calls: 0,
                started_at: granted.started_at.clone(),
                reads: 0,
                recordings: 0,
                transactions_recorded: 0,
                last_recording: None,
                assets_added: 0,
                last_asset: None,
            }]
        );
        assert!(connections.requests().is_empty());
        assert_eq!(changes.load(Ordering::SeqCst), 3);

        let asking = Arc::clone(&connections);
        let asked = tokio::spawn(async move { asking.ask("other").await });
        let request = waiting(&connections).await;
        assert!(connections.answer(request.id, false));
        assert!(asked.await.expect("asked").is_none());
        assert_eq!(connections.sessions().len(), 1);
        assert!(!connections.answer(request.id, true));
    }

    // AGT-030 — a client that goes away before the answer takes its request with it.
    #[tokio::test]
    async fn agt_030_a_request_whose_asker_left_is_withdrawn() {
        let (connections, _) = counted();
        let asking = Arc::clone(&connections);
        let asked = tokio::spawn(async move { asking.ask("claude-code").await });
        waiting(&connections).await;

        asked.abort();
        let _ = asked.await;

        assert!(connections.requests().is_empty());
        assert!(connections.sessions().is_empty());
    }

    // AGT-033 / AGT-034 / AGT-035 — calls are counted per session; ending a session tells
    // the connection serving it; ending all refuses who waits and ends who is connected.
    #[tokio::test]
    async fn agt_034_ending_a_session_tells_its_connection() {
        let (connections, _) = counted();
        let asking = Arc::clone(&connections);
        let asked = tokio::spawn(async move { asking.ask("claude-code").await });
        let request = waiting(&connections).await;
        connections.answer(request.id, true);
        let granted = asked.await.expect("asked").expect("granted");

        connections.count_call(granted.session_id);
        connections.count_call(granted.session_id);
        connections.count_call(999);
        assert_eq!(connections.sessions()[0].calls, 2);

        assert!(connections.end(granted.session_id));
        tokio::time::timeout(Duration::from_secs(1), granted.ended.notified())
            .await
            .expect("the connection is told");
        assert!(connections.sessions().is_empty());
        assert!(!connections.end(granted.session_id));

        let asking = Arc::clone(&connections);
        let waiting_one = tokio::spawn(async move { asking.ask("waits").await });
        waiting(&connections).await;
        connections.end_all();
        assert!(waiting_one.await.expect("asked").is_none());
        assert!(connections.requests().is_empty());
        connections.end_all();
    }

    // AGT-047 — a session records up to its limit and no further.
    #[tokio::test]
    async fn agt_047_a_session_records_up_to_its_limit() {
        let (connections, _) = counted();
        let asking = Arc::clone(&connections);
        let asked = tokio::spawn(async move { asking.ask("claude-code").await });
        let request = waiting(&connections).await;
        connections.answer(request.id, true);
        let granted = asked.await.expect("asked").expect("granted");
        assert_eq!(granted.client, "claude-code");
        assert_eq!(granted.key.len(), 36);

        for _ in 0..MAX_RECORDINGS {
            assert!(granted.may_record_one_more());
            granted.count_recording();
        }
        assert!(!granted.may_record_one_more());
        // The count is the session's, shared by every handle on it.
        assert!(!granted.clone().may_record_one_more());
    }

    // AGT-035 / AGT-038 — nobody is asked for while agents are not admitted, nor beyond the
    // number that may wait at once; a yes that arrives after the setting went off opens
    // no session.
    #[tokio::test]
    async fn agt_038_nobody_is_asked_for_beyond_the_limit_or_while_not_admitted() {
        let (connections, changes) = counted();
        let mut waiting_ones = Vec::new();
        for index in 0..MAX_REQUESTS {
            let asking = Arc::clone(&connections);
            waiting_ones.push(tokio::spawn(async move {
                asking.ask(&format!("agent-{index}")).await
            }));
        }
        for _ in 0..200 {
            if connections.requests().len() == MAX_REQUESTS {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(connections.requests().len(), MAX_REQUESTS);
        assert!(connections.ask("one-too-many").await.is_none());
        assert_eq!(connections.requests().len(), MAX_REQUESTS);

        connections.end_all();
        for waiting_one in waiting_ones {
            assert!(waiting_one.await.expect("asked").is_none());
        }
        let before = changes.load(Ordering::SeqCst);
        assert!(connections
            .ask("after-the-setting-went-off")
            .await
            .is_none());
        assert_eq!(changes.load(Ordering::SeqCst), before, "nothing was shown");

        connections.notify();
        assert_eq!(changes.load(Ordering::SeqCst), before + 1);
    }
}
