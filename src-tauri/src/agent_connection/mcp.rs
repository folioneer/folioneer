//! The Model Context Protocol as the bridge speaks it to an agent client (AGT-010): JSON-RPC
//! messages, one per line, on the program's standard input and output. Only what serving
//! tools needs is answered: `initialize`, `ping`, `tools/list` and `tools/call`.

use serde_json::{json, Value};

use super::definitions::ToolDefinition;
use super::wire::shown_name;

/// The protocol version answered when the client asks for none.
const PROTOCOL_VERSION: &str = "2025-06-18";
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INVALID_REQUEST: i64 = -32600;

/// What calling a tool gave: the text an agent reads, and whether it is a refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutcome {
    /// The result, or the reason of the refusal.
    pub text: String,
    /// Whether the call was refused.
    pub refused: bool,
}

impl ToolOutcome {
    /// A refusal with `reason`.
    pub fn refusal(reason: impl Into<String>) -> Self {
        Self {
            text: reason.into(),
            refused: true,
        }
    }
}

/// Who runs the tools: the running application, reached by the bridge.
#[async_trait::async_trait]
pub trait ToolHost: Send {
    /// Calls `tool` with `arguments` for the agent client named `client`.
    async fn call(&mut self, client: &str, tool: &str, arguments: Value) -> ToolOutcome;
}

/// One agent client's conversation: what it said it is, and the tools it is offered.
pub struct Conversation {
    client: String,
    tools: Vec<ToolDefinition>,
}

impl Conversation {
    /// A conversation offering `tools`.
    pub fn new(tools: Vec<ToolDefinition>) -> Self {
        Self {
            client: super::wire::UNNAMED_AGENT.to_string(),
            tools,
        }
    }

    /// Answers one message of the client: `None` for a notification, which takes no answer.
    pub async fn answer(&mut self, message: &Value, host: &mut dyn ToolHost) -> Option<Value> {
        let id = message.get("id").cloned();
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return id.map(|id| failure(id, INVALID_REQUEST, "not a request"));
        };
        let id = id?;
        let params = message.get("params");
        Some(match method {
            "initialize" => {
                self.client = client_name(params);
                success(
                    id,
                    json!({
                        "protocolVersion": params
                            .and_then(|params| params.get("protocolVersion"))
                            .and_then(Value::as_str)
                            .unwrap_or(PROTOCOL_VERSION),
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": "folioneer", "version": env!("CARGO_PKG_VERSION") },
                    }),
                )
            }
            "ping" => success(id, json!({})),
            "tools/list" => success(
                id,
                json!({
                    "tools": self
                        .tools
                        .iter()
                        .map(|tool| json!({
                            "name": tool.name,
                            "description": tool.description,
                            "inputSchema": tool.input_schema,
                        }))
                        .collect::<Vec<_>>(),
                }),
            ),
            "tools/call" => {
                let name = params
                    .and_then(|params| params.get("name"))
                    .and_then(Value::as_str);
                match name.filter(|name| self.tools.iter().any(|tool| tool.name == *name)) {
                    None => failure(id, INVALID_PARAMS, "no such tool"),
                    Some(name) => {
                        let arguments = params
                            .and_then(|params| params.get("arguments"))
                            .cloned()
                            .unwrap_or_else(|| json!({}));
                        let outcome = host.call(&self.client, name, arguments).await;
                        success(
                            id,
                            json!({
                                "content": [{ "type": "text", "text": outcome.text }],
                                "isError": outcome.refused,
                            }),
                        )
                    }
                }
            }
            _ => failure(id, METHOD_NOT_FOUND, "method not found"),
        })
    }
}

/// The name the client gave for itself, cleaned as it will be shown (AGT-037).
fn client_name(params: Option<&Value>) -> String {
    shown_name(
        params
            .and_then(|params| params.get("clientInfo"))
            .and_then(|info| info.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn failure(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Recorded(Vec<(String, String, Value)>);

    #[async_trait::async_trait]
    impl ToolHost for Recorded {
        async fn call(&mut self, client: &str, tool: &str, arguments: Value) -> ToolOutcome {
            self.0
                .push((client.to_string(), tool.to_string(), arguments));
            ToolOutcome {
                text: "done".to_string(),
                refused: false,
            }
        }
    }

    fn conversation() -> Conversation {
        Conversation::new(vec![ToolDefinition {
            name: "list_accounts",
            description: "The accounts.",
            input_schema: json!({ "type": "object" }),
        }])
    }

    // AGT-010 — the bridge introduces itself, lists its tools and answers a ping; a
    // notification takes no answer.
    #[tokio::test]
    async fn agt_010_the_bridge_introduces_itself_and_lists_its_tools() {
        let mut host = Recorded(vec![]);
        let mut conversation = conversation();

        let hello = conversation
            .answer(
                &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": { "protocolVersion": "2025-03-26", "clientInfo": { "name": "claude-code" } } }),
                &mut host,
            )
            .await
            .expect("answer");
        assert_eq!(hello["id"], 1);
        assert_eq!(hello["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(hello["result"]["serverInfo"]["name"], "folioneer");
        assert_eq!(hello["result"]["capabilities"], json!({ "tools": {} }));

        let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert_eq!(conversation.answer(&initialized, &mut host).await, None);

        let ping = json!({ "jsonrpc": "2.0", "id": "p", "method": "ping" });
        assert_eq!(
            conversation.answer(&ping, &mut host).await,
            Some(json!({ "jsonrpc": "2.0", "id": "p", "result": {} }))
        );

        let listed = conversation
            .answer(
                &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
                &mut host,
            )
            .await
            .expect("answer");
        assert_eq!(
            listed["result"]["tools"],
            json!([{ "name": "list_accounts", "description": "The accounts.", "inputSchema": { "type": "object" } }])
        );
        assert!(host.0.is_empty());
    }

    // AGT-010 — a tool call goes to the application under the client's name; a tool that is
    // not offered never does, nor does a method the bridge does not serve.
    #[tokio::test]
    async fn agt_010_only_an_offered_tool_reaches_the_application() {
        let mut host = Recorded(vec![]);
        let mut conversation = conversation();
        conversation
            .answer(
                &json!({ "id": 1, "method": "initialize", "params": { "clientInfo": { "name": "claude-code" } } }),
                &mut host,
            )
            .await;

        let called = conversation
            .answer(
                &json!({ "id": 2, "method": "tools/call", "params": { "name": "list_accounts", "arguments": { "a": 1 } } }),
                &mut host,
            )
            .await
            .expect("answer");
        assert_eq!(
            called["result"],
            json!({ "content": [{ "type": "text", "text": "done" }], "isError": false })
        );
        assert_eq!(
            host.0,
            vec![(
                "claude-code".to_string(),
                "list_accounts".to_string(),
                json!({ "a": 1 })
            )]
        );

        for refused in [
            json!({ "id": 3, "method": "tools/call", "params": { "name": "delete_everything" } }),
            json!({ "id": 4, "method": "tools/call" }),
            json!({ "id": 5, "method": "resources/list" }),
            json!({ "id": 6 }),
        ] {
            let answer = conversation
                .answer(&refused, &mut host)
                .await
                .expect("answer");
            assert!(answer["error"]["code"].is_i64(), "{refused}");
        }
        assert_eq!(host.0.len(), 1);
        assert_eq!(
            conversation
                .answer(&json!({ "nothing": true }), &mut host)
                .await,
            None
        );
    }

    // AGT-037 — the name an agent gives is text this program did not write: it is shown
    // without control or invisible characters, bounded, and never empty.
    #[test]
    fn agt_037_a_client_name_is_bounded_and_printable() {
        use crate::agent_connection::wire::{AGENT_NAME_LENGTH, UNNAMED_AGENT};

        let named = |name: &str| client_name(Some(&json!({ "clientInfo": { "name": name } })));
        assert_eq!(named("claude-code"), "claude-code");
        assert_eq!(named("  evil\u{1b}[31m\nname "), "evil[31mname");
        assert_eq!(named(&"x".repeat(500)).chars().count(), AGENT_NAME_LENGTH);
        assert_eq!(
            named("claude\u{202E}edoc\u{200B}-\u{FEFF}x"),
            "claudeedoc-x"
        );
        assert_eq!(named(" \n "), UNNAMED_AGENT);
        assert_eq!(client_name(None), UNNAMED_AGENT);
    }
}
