//! The owner's setting "Allow agents to connect" (AGT-022): off unless the owner switched
//! it on, kept on this computer across restarts, and the only thing that opens the channel.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::task::JoinHandle;

use crate::core::BACKEND;

use super::channel;
use super::connections::AgentConnections;
use super::server::ToolRunner;

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
    #[cfg_attr(not(unix), allow(dead_code))]
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

    #[cfg(unix)]
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

    #[cfg(not(unix))]
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
#[cfg(unix)]
async fn accept(
    listener: tokio::net::UnixListener,
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
            Ok((stream, _)) => {
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
