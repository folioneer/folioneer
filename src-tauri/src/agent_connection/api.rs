// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

//! The window's commands over the agent connection (AGT-022, AGT-032 to AGT-036): the
//! owner's setting, the answer to a connection request, and the disconnect.

use std::sync::Arc;

use serde::Serialize;
use specta::Type;
use tauri::State;

use super::channel;
use super::connections::{AgentConnectionRequest, AgentSession};
use super::gate::{AgentGate, GateError};

/// Failures of the agent connection commands.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Type)]
#[serde(tag = "code")]
pub enum AgentConnectionError {
    /// This system has no agent connection yet (AGT-023).
    #[error("The agent connection is not available on this system")]
    NoAgentChannel,
    /// The setting or the channel could not be written; the cause is in the log.
    #[error("The agent connection could not be changed")]
    AgentChannelFailed,
    /// The request was already answered, or its agent client went away.
    #[error("No such connection request")]
    ConnectionRequestGone,
    /// The session already ended.
    #[error("No such agent session")]
    SessionAlreadyEnded,
}

impl From<GateError> for AgentConnectionError {
    fn from(error: GateError) -> Self {
        match error {
            GateError::Unavailable => Self::NoAgentChannel,
            GateError::Failed => Self::AgentChannelFailed,
        }
    }
}

/// What the window shows of the agent connection (AGT-036).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct AgentConnectionState {
    /// Whether this system has an agent connection (AGT-023).
    pub available: bool,
    /// Whether the owner allows agents to connect (AGT-022).
    pub allowed: bool,
    /// The user this application runs as — the only one whose programs can connect
    /// (AGT-021); absent when the system does not say.
    pub user: Option<String>,
    /// The agent clients waiting for the owner's answer, oldest first.
    pub requests: Vec<AgentConnectionRequest>,
    /// The agent clients connected, oldest first.
    pub sessions: Vec<AgentSession>,
}

/// The state of `gate`, as the window reads it.
pub fn state_of(gate: &AgentGate) -> AgentConnectionState {
    AgentConnectionState {
        available: channel::AVAILABLE,
        allowed: gate.is_allowed(),
        user: std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .ok()
            .filter(|user| !user.trim().is_empty()),
        requests: gate.connections().requests(),
        sessions: gate.connections().sessions(),
    }
}

/// AGT-036 — the setting, who asks to connect and who is connected. Infallible: it reads
/// what the application holds in memory.
#[tauri::command]
#[specta::specta]
pub fn get_agent_connection_state(gate: State<'_, Arc<AgentGate>>) -> AgentConnectionState {
    state_of(&gate)
}

/// AGT-022 — the owner allows or stops allowing agents to connect.
#[tauri::command]
#[specta::specta]
pub async fn set_agents_allowed(
    gate: State<'_, Arc<AgentGate>>,
    allowed: bool,
) -> Result<AgentConnectionState, AgentConnectionError> {
    gate.allow(allowed)?;
    Ok(state_of(&gate))
}

/// AGT-032 — the owner's answer to a connection request.
#[tauri::command]
#[specta::specta]
pub fn answer_agent_connection(
    gate: State<'_, Arc<AgentGate>>,
    request_id: u32,
    allow: bool,
) -> Result<(), AgentConnectionError> {
    if gate.connections().answer(request_id, allow) {
        Ok(())
    } else {
        Err(AgentConnectionError::ConnectionRequestGone)
    }
}

/// AGT-034 — the owner disconnects an agent client.
#[tauri::command]
#[specta::specta]
pub fn disconnect_agent(
    gate: State<'_, Arc<AgentGate>>,
    session_id: u32,
) -> Result<(), AgentConnectionError> {
    if gate.connections().end(session_id) {
        Ok(())
    } else {
        Err(AgentConnectionError::SessionAlreadyEnded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // The wire shape of the errors is `{ "code": … }`, as the error model asks.
    #[test]
    fn the_errors_serialise_with_a_code() {
        for (error, code) in [
            (
                AgentConnectionError::from(GateError::Unavailable),
                "NoAgentChannel",
            ),
            (
                AgentConnectionError::from(GateError::Failed),
                "AgentChannelFailed",
            ),
            (
                AgentConnectionError::ConnectionRequestGone,
                "ConnectionRequestGone",
            ),
            (
                AgentConnectionError::SessionAlreadyEnded,
                "SessionAlreadyEnded",
            ),
        ] {
            assert_eq!(
                serde_json::to_value(&error).expect("json"),
                json!({ "code": code })
            );
        }
    }
}
