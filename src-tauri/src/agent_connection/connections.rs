//! Who is connected and who asks to be (AGT-030 to AGT-035): the owner's answers and the
//! sessions they open, held in memory by the running application and gone when it closes.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use specta::Type;
use tokio::sync::{oneshot, Notify};

use crate::context::account::TransactionType;

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
    /// The one it recorded last among them.
    pub last_recording: Option<LastRecording>,
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

/// What a session did, shared by the connection that serves it and by what the window is
/// told of it.
#[derive(Debug, Default)]
struct Facts {
    reads: AtomicU32,
    /// How many of its recordings still exist, and the last of them: read from the
    /// portfolio after each change, so the two always agree.
    recordings: Mutex<(u32, Option<LastRecording>)>,
}

impl Facts {
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

    /// AGT-052 — one call that read the portfolio was answered.
    pub fn count_read(&self) {
        self.facts.reads.fetch_add(1, Ordering::SeqCst);
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
        AgentSession {
            reads: self.facts.reads.load(Ordering::SeqCst),
            recordings,
            last_recording,
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
                    last_recording: None,
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
            position.map(|position| state.open.remove(position))
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
        let (pending, open) = {
            let mut state = self.state();
            state.admitting = false;
            (
                std::mem::take(&mut state.pending),
                std::mem::take(&mut state.open),
            )
        };
        if pending.is_empty() && open.is_empty() {
            return;
        }
        for pending in pending {
            let _ = pending.answer.send(false);
        }
        for open in open {
            open.ended.notify_one();
        }
        (self.changed)();
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
                last_recording: None,
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
