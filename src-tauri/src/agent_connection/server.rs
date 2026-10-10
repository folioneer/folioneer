//! The running application's side of the channel (AGT-020, AGT-030, AGT-040): one
//! connection per bridge, which asks to connect once and then calls tools until either
//! side closes it or the owner disconnects it.

use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, BufReader};

use crate::core::BACKEND;

use super::connections::{AgentConnections, Granted};
use super::tools::{AgentTools, Refusal};
use super::wire::{read_frame, shown_name, write_frame, Answer, Request};

/// AGT-038 — how long a connection may stay without asking to connect.
pub const CONNECT_WITHIN: std::time::Duration = std::time::Duration::from_secs(30);

/// Who runs a tool for a connected agent.
#[async_trait::async_trait]
pub trait ToolRunner: Send + Sync {
    /// Runs `tool` with `arguments` for the session `granted`.
    async fn run(&self, granted: &Granted, tool: &str, arguments: &Value)
        -> Result<Value, Refusal>;
}

#[async_trait::async_trait]
impl ToolRunner for AgentTools {
    async fn run(
        &self,
        granted: &Granted,
        tool: &str,
        arguments: &Value,
    ) -> Result<Value, Refusal> {
        self.call(granted, tool, arguments).await
    }
}

fn refused(code: &str, message: &str) -> Answer {
    Answer::Refused {
        code: code.to_string(),
        message: message.to_string(),
    }
}

/// Serves one bridge over `stream` until it closes, sends what is no request, or the owner
/// ends its session. Nothing is run before the owner allowed the connection (AGT-031).
pub async fn serve_connection<S>(
    stream: S,
    connections: Arc<AgentConnections>,
    tools: Arc<dyn ToolRunner>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reading, mut writing) = tokio::io::split(stream);
    let mut reading = BufReader::new(reading);
    let mut session: Option<Granted> = None;
    loop {
        let request = match &session {
            // AGT-038 — a connection that asks for nothing is not kept.
            None => match tokio::time::timeout(CONNECT_WITHIN, read_frame(&mut reading)).await {
                Ok(request) => request,
                Err(_) => break,
            },
            Some(granted) => tokio::select! {
                biased;
                // AGT-034 — the owner disconnected: the connection closes under the bridge,
                // before anything it already sent is read.
                () = granted.ended.notified() => break,
                request = read_frame::<_, Request>(&mut reading) => request,
            },
        };
        let Ok(Some(request)) = request else {
            break;
        };
        let answer = match (request, &session) {
            (Request::Connect { .. }, Some(_)) => Answer::Granted,
            (Request::Connect { client }, None) => {
                // AGT-037 — the name is cleaned here too: a program may speak to the
                // channel without the bridge.
                let client = shown_name(&client);
                let granted = tokio::select! {
                    granted = connections.ask(&client) => granted,
                    // The bridge left, or spoke out of turn, while the owner was asked.
                    _ = reading.fill_buf() => break,
                };
                match granted {
                    Some(granted) => {
                        tracing::info!(target: BACKEND, client = %client, session = granted.session_id, "agent connection allowed");
                        session = Some(granted);
                        Answer::Granted
                    }
                    None => {
                        tracing::info!(target: BACKEND, client = %client, "agent connection refused");
                        refused("ConnectionRefused", "the connection was refused")
                    }
                }
            }
            (Request::Call { .. }, None) => refused("NotConnected", "not connected"),
            (Request::Call { tool, arguments }, Some(granted)) => {
                connections.count_call(granted.session_id);
                let result = tools.run(granted, &tool, &arguments).await;
                // AGT-052 — what the call read or recorded shows in the window.
                connections.notify();
                // AGT-041 — every call is written to the log with its tool.
                tracing::info!(target: BACKEND, session = granted.session_id, tool = %tool, refused = result.is_err(), "agent tool called");
                match result {
                    Ok(content) => Answer::Result { content },
                    Err(refusal) => Answer::Refused {
                        code: refusal.code,
                        message: refusal.message,
                    },
                }
            }
        };
        if write_frame(&mut writing, &answer).await.is_err() {
            break;
        }
    }
    if let Some(granted) = session {
        connections.end(granted.session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;
    use tokio::io::{duplex, DuplexStream};

    struct Echo;

    #[async_trait::async_trait]
    impl ToolRunner for Echo {
        async fn run(
            &self,
            _granted: &Granted,
            tool: &str,
            arguments: &Value,
        ) -> Result<Value, Refusal> {
            if tool == "list_accounts" {
                Ok(json!({ "tool": tool, "arguments": arguments }))
            } else {
                Err(Refusal {
                    code: "UnknownTool".to_string(),
                    message: "no such tool".to_string(),
                })
            }
        }
    }

    struct Bridge {
        channel: BufReader<DuplexStream>,
        connections: Arc<AgentConnections>,
        served: tokio::task::JoinHandle<()>,
    }

    fn bridge() -> Bridge {
        let (bridge_end, application_end) = duplex(64 * 1024);
        let connections = Arc::new(AgentConnections::new(|| {}));
        connections.admit();
        let served = tokio::spawn(serve_connection(
            application_end,
            Arc::clone(&connections),
            Arc::new(Echo),
        ));
        Bridge {
            channel: BufReader::new(bridge_end),
            connections,
            served,
        }
    }

    impl Bridge {
        async fn send(&mut self, request: &Request) {
            write_frame(&mut self.channel, request).await.expect("sent");
        }

        async fn answer(&mut self) -> Option<Answer> {
            tokio::time::timeout(Duration::from_secs(2), read_frame(&mut self.channel))
                .await
                .expect("an answer in time")
                .expect("readable")
        }

        async fn owner_answers(&self, allow: bool) {
            for _ in 0..200 {
                if let Some(request) = self.connections.requests().into_iter().next() {
                    self.connections.answer(request.id, allow);
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            panic!("no request arrived");
        }
    }

    fn connect() -> Request {
        Request::Connect {
            client: "claude-code".to_string(),
        }
    }

    fn call(tool: &str) -> Request {
        Request::Call {
            tool: tool.to_string(),
            arguments: json!({ "a": 1 }),
        }
    }

    // AGT-031 — nothing is run for a bridge the owner has not allowed: a call before the
    // connection, or after a refusal, is refused and reaches no tool.
    #[tokio::test]
    async fn agt_031_no_tool_runs_before_the_owner_allows() {
        let mut bridge = bridge();

        bridge.send(&call("list_accounts")).await;
        assert_eq!(
            bridge.answer().await,
            Some(refused("NotConnected", "not connected"))
        );

        bridge.send(&connect()).await;
        bridge.owner_answers(false).await;
        assert_eq!(
            bridge.answer().await,
            Some(refused("ConnectionRefused", "the connection was refused"))
        );
        bridge.send(&call("list_accounts")).await;
        assert_eq!(
            bridge.answer().await,
            Some(refused("NotConnected", "not connected"))
        );
        assert!(bridge.connections.sessions().is_empty());
    }

    // AGT-030 / AGT-040 / AGT-033 — once allowed, calls run and are counted; a tool's
    // refusal travels with its code; asking to connect again changes nothing.
    #[tokio::test]
    async fn agt_040_an_allowed_bridge_calls_tools_and_its_calls_are_counted() {
        let mut bridge = bridge();
        bridge.send(&connect()).await;
        bridge.owner_answers(true).await;
        assert_eq!(bridge.answer().await, Some(Answer::Granted));

        bridge.send(&call("list_accounts")).await;
        assert_eq!(
            bridge.answer().await,
            Some(Answer::Result {
                content: json!({ "tool": "list_accounts", "arguments": { "a": 1 } })
            })
        );
        bridge.send(&call("delete_everything")).await;
        assert_eq!(
            bridge.answer().await,
            Some(refused("UnknownTool", "no such tool"))
        );
        bridge.send(&connect()).await;
        assert_eq!(bridge.answer().await, Some(Answer::Granted));

        let sessions = bridge.connections.sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].calls, 2);
    }

    // AGT-034 — the owner's disconnect closes the channel under the bridge; a bridge that
    // goes away ends its session.
    #[tokio::test]
    async fn agt_034_a_disconnect_closes_the_channel_and_a_departure_ends_the_session() {
        let mut bridge = bridge();
        bridge.send(&connect()).await;
        bridge.owner_answers(true).await;
        bridge.answer().await;
        let session_id = bridge.connections.sessions()[0].id;

        assert!(bridge.connections.end(session_id));
        assert_eq!(bridge.answer().await, None);
        bridge.served.await.expect("served");

        let mut leaving = self::bridge();
        leaving.send(&connect()).await;
        leaving.owner_answers(true).await;
        leaving.answer().await;
        let connections = Arc::clone(&leaving.connections);
        drop(leaving.channel);
        leaving.served.await.expect("served");
        assert!(connections.sessions().is_empty());
    }

    // AGT-037 / AGT-038 — a name sent without the bridge is cleaned before it is shown; a
    // connection that asks for nothing is closed.
    #[tokio::test(start_paused = true)]
    async fn agt_037_a_name_sent_past_the_bridge_is_cleaned_and_an_idle_connection_closed() {
        let mut direct = bridge();
        direct
            .send(&Request::Connect {
                client: format!("evil\u{202E}\n{}", "x".repeat(5_000)),
            })
            .await;
        let mut shown = None;
        for _ in 0..200 {
            shown = direct.connections.requests().into_iter().next();
            if shown.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let shown = shown.expect("a request").client;
        assert!(shown.starts_with("evilx"));
        assert_eq!(shown.chars().count(), 60);

        let mut idle = self::bridge();
        tokio::time::sleep(CONNECT_WITHIN + Duration::from_secs(1)).await;
        assert_eq!(idle.answer().await, None);
        idle.served.await.expect("served");
    }

    // AGT-030 — a bridge that leaves while the owner is asked takes its request with it;
    // a line that is no request ends the connection.
    #[tokio::test]
    async fn agt_030_a_bridge_that_leaves_while_asked_withdraws_its_request() {
        let mut bridge = bridge();
        bridge.send(&connect()).await;
        for _ in 0..200 {
            if !bridge.connections.requests().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(bridge.connections.requests().len(), 1);
        let connections = Arc::clone(&bridge.connections);
        drop(bridge.channel);
        bridge.served.await.expect("served");
        assert!(connections.requests().is_empty());

        let mut garbled = self::bridge();
        tokio::io::AsyncWriteExt::write_all(&mut garbled.channel, b"{\"type\":\"sql\"}\n")
            .await
            .expect("sent");
        assert_eq!(garbled.answer().await, None);
    }
}
