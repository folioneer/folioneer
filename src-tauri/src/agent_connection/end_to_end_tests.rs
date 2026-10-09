//! The agent connection from end to end: an agent client's messages enter the bridge, cross
//! the channel, wait for the owner in the running application and are answered from a real
//! portfolio.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::context::account::{AccountServiceContract, UpdateFrequency};
use crate::context::asset::{AssetClass, AssetServiceContract, CreateAssetDTO};
use crate::core::Database;
use crate::shared::infrastructure::container::AppContainer;
use crate::use_cases::account_creation::AccountCreationUseCase;

use super::bridge::{self, Channel, Connector, NOT_ALLOWED, NOT_OPEN};
use super::channel;
use super::connections::AgentConnections;
use super::gate::{AgentGate, GateError};
use super::tools::AgentTools;

const M: i64 = 1_000_000;

/// The application running over a portfolio: the account "PEA" with 5,000 EUR of cash and
/// 10 shares of "Amundi MSCI World" (CW8) bought for 4,000 EUR.
async fn application(data_dir: &Path) -> Arc<AgentGate> {
    let database = Database::new(data_dir.to_path_buf())
        .await
        .expect("database");
    let container = AppContainer::for_headless_writes(database.pool);
    let accounts: Arc<dyn AccountServiceContract> = Arc::clone(&container.account_service) as _;
    let assets: Arc<dyn AssetServiceContract> = Arc::clone(&container.asset_service) as _;
    if accounts.get_all().await.expect("accounts").is_empty() {
        let account = AccountCreationUseCase::new(Arc::clone(&accounts), Arc::clone(&assets))
            .create(
                "PEA".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .expect("account");
        container
            .account_service
            .record_deposit(&account.id, "2026-01-02".to_string(), 5_000 * M, None)
            .await
            .expect("deposit");
        container
            .asset_service
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "Amundi MSCI World".to_string(),
                reference: "CW8".to_string(),
                isin: None,
                class: AssetClass::ETF,
                currency: "EUR".to_string(),
                risk_level: 3,
                category_id: "default-uncategorized".to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .expect("asset");
    }
    Arc::new(AgentGate::new(
        data_dir.to_path_buf(),
        Arc::new(AgentConnections::new(|| {})),
        Arc::new(AgentTools::new(
            accounts,
            assets,
            Arc::clone(&container.currency_service),
        )),
    ))
}

fn reaching(data_dir: &Path) -> Connector {
    let data_dir = data_dir.to_path_buf();
    Box::new(move || {
        let data_dir = data_dir.clone();
        Box::pin(async move { channel::reach(&data_dir).await })
    })
}

fn call(id: u32, tool: &str, arguments: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments } })
    .to_string()
}

/// What an agent client is answered when it sends `lines` to the bridge.
async fn answers(data_dir: &Path, lines: &[String]) -> Vec<Value> {
    let input = format!("{}\n", lines.join("\n"));
    let mut output = Vec::new();
    bridge::serve(input.as_bytes(), &mut output, reaching(data_dir))
        .await
        .expect("served");
    String::from_utf8(output)
        .expect("text")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect()
}

/// The owner answers the next connection request.
fn owner_answers(gate: &Arc<AgentGate>, allow: bool) -> tokio::task::JoinHandle<String> {
    let gate = Arc::clone(gate);
    tokio::spawn(async move {
        for _ in 0..400 {
            if let Some(request) = gate.connections().requests().into_iter().next() {
                gate.connections().answer(request.id, allow);
                return request.client;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("no connection request arrived");
    })
}

fn text(answer: &Value) -> &str {
    answer["result"]["content"][0]["text"]
        .as_str()
        .expect("text")
}

fn result(answer: &Value) -> Value {
    assert_eq!(answer["result"]["isError"], false, "{answer}");
    serde_json::from_str(text(answer)).expect("a JSON result")
}

// AGT-022 / AGT-011 — with the setting off no channel exists, and an agent is told the
// application is not open; switched on, the setting is kept for the next start; switched
// off, the channel is gone again.
#[tokio::test]
async fn agt_022_no_channel_exists_unless_the_owner_allows_agents() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    let listing = [call(1, "list_accounts", json!({}))];

    assert!(!gate.is_allowed());
    gate.open_if_allowed();
    assert!(!channel::socket_path(data_dir.path()).exists());
    assert_eq!(text(&answers(data_dir.path(), &listing).await[0]), NOT_OPEN);

    assert_eq!(gate.allow(true), Ok(()));
    assert_eq!(gate.allow(true), Ok(()));
    // AGT-024 — a second application over the same folder cannot take the channel of the
    // first, and removes nothing when it closes.
    let second = application(data_dir.path()).await;
    assert_eq!(second.allow(true), Err(GateError::Failed));
    drop(second);
    assert!(channel::socket_path(data_dir.path()).exists());
    assert!(channel::reach(data_dir.path()).await.is_some());
    assert!(gate.is_allowed());
    assert!(channel::socket_path(data_dir.path()).exists());

    drop(gate);
    assert!(!channel::socket_path(data_dir.path()).exists());
    let restarted = application(data_dir.path()).await;
    assert!(restarted.is_allowed());
    restarted.open_if_allowed();
    assert!(channel::socket_path(data_dir.path()).exists());

    assert_eq!(restarted.allow(false), Ok(()));
    assert_eq!(restarted.allow(false), Ok(()));
    assert!(!restarted.is_allowed());
    assert!(!channel::socket_path(data_dir.path()).exists());
    assert_eq!(text(&answers(data_dir.path(), &listing).await[0]), NOT_OPEN);
}

// AGT-030 / AGT-040 — an agent's first call waits for the owner; once allowed, the read
// tools answer from the running application's portfolio.
#[tokio::test]
async fn agt_040_the_read_tools_answer_from_the_running_application() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let owner = owner_answers(&gate, true);

    let answered = answers(
        data_dir.path(),
        &[
            json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
                "params": { "clientInfo": { "name": "claude-code" } } })
            .to_string(),
            call(1, "list_accounts", json!({})),
            call(2, "list_assets", json!({})),
            call(3, "portfolio_summary", json!({})),
            call(4, "list_holdings", json!({ "account": "pea" })),
            call(
                5,
                "list_holdings",
                json!({ "account": "PEA", "as_of": "2026-01-01" }),
            ),
        ],
    )
    .await;

    assert_eq!(owner.await.expect("owner"), "claude-code");
    assert_eq!(
        result(&answered[1]),
        json!([{ "name": "PEA", "currency": "EUR" }])
    );
    assert_eq!(
        result(&answered[2]),
        json!([{ "name": "Amundi MSCI World", "reference": "CW8", "kind": "Custom",
            "class": "ETF", "currency": "EUR", "isin": null, "exchange": null,
            "archived": false }])
    );
    let summary = result(&answered[3]);
    assert_eq!(summary["summaries"][0]["name"], "PEA");
    assert_eq!(summary["summaries"][0]["total_global_value"], 5_000 * M);
    let holdings = result(&answered[4]);
    assert_eq!(holdings["account_name"], "PEA");
    assert_eq!(holdings["total_global_value"], 5_000 * M);
    let before_the_deposit = result(&answered[5]);
    assert_eq!(before_the_deposit["total_global_value"], 0);
}

// AGT-042 / AGT-043 — a refusal names only what the agent sent: an unknown account is not answered
// with the accounts that exist, and a tool that is not offered never reaches the application.
#[tokio::test]
async fn agt_042_a_refusal_names_only_what_was_sent() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let owner = owner_answers(&gate, true);

    let answered = answers(
        data_dir.path(),
        &[
            call(1, "list_holdings", json!({ "account": "CTO" })),
            call(2, "list_holdings", json!({})),
            call(
                3,
                "list_holdings",
                json!({ "account": "PEA", "as_of": "tomorrow" }),
            ),
            call(4, "delete_everything", json!({})),
        ],
    )
    .await;
    owner.await.expect("owner");

    assert_eq!(answered[0]["result"]["isError"], true);
    assert_eq!(text(&answered[0]), "no account named \"CTO\"");
    assert!(!answered[0].to_string().contains("PEA"));
    assert_eq!(text(&answered[1]), "account is missing");
    assert_eq!(
        text(&answered[2]),
        "as_of is not a past date written YYYY-MM-DD"
    );
    assert!(answered[3]["error"]["code"].is_i64());
    // AGT-034 — the bridge left with its client: its session ends.
    for _ in 0..400 {
        if gate.connections().sessions().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(gate.connections().sessions().is_empty());
}

// AGT-032 / AGT-035 — a refusal gives the agent nothing; switching the setting off while
// an agent waits refuses it too.
#[tokio::test]
async fn agt_032_a_refused_agent_is_given_nothing() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");

    let owner = owner_answers(&gate, false);
    let refused = answers(data_dir.path(), &[call(1, "portfolio_summary", json!({}))]).await;
    owner.await.expect("owner");
    assert_eq!(refused[0]["result"]["isError"], true);
    assert_eq!(text(&refused[0]), NOT_ALLOWED);

    let switching = Arc::clone(&gate);
    let switched_off = tokio::spawn(async move {
        for _ in 0..400 {
            if !switching.connections().requests().is_empty() {
                return switching.allow(false);
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        Err(GateError::Failed)
    });
    let cut = answers(data_dir.path(), &[call(1, "portfolio_summary", json!({}))]).await;
    assert_eq!(switched_off.await.expect("switched"), Ok(()));
    assert_eq!(cut[0]["result"]["isError"], true);
    assert!(!cut[0].to_string().contains("PEA"));
}

// ADR-023 — nothing in the agent connection opens a network socket, and the bridge reads
// neither the database nor a service.
#[test]
fn adr_023_the_agent_connection_opens_no_network_socket() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agent_connection");
    let mut scanned = 0;
    for entry in std::fs::read_dir(&folder).expect("the module's folder") {
        let path = entry.expect("entry").path();
        let file = path
            .file_name()
            .expect("name")
            .to_string_lossy()
            .to_string();
        // This file names what it forbids.
        if file == "end_to_end_tests.rs" {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("source");
        for network in [
            "TcpListener",
            "TcpStream",
            "UdpSocket",
            "std::net",
            "hyper",
            "reqwest",
            "axum",
            "warp",
            "tiny_http",
        ] {
            assert!(!source.contains(network), "{file} names {network}");
        }
        scanned += 1;
    }
    assert!(scanned >= 10, "every file of the module is scanned");
}

// ADR-023 — the bridge is made of files that hold no data and reach none: it uses nothing
// else of the module, and none of them names the database, a service or a file.
#[test]
fn adr_023_the_bridge_is_made_of_files_that_reach_no_data() {
    // The bridge itself: what comes before its tests.
    let bridge = include_str!("bridge.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap_or_default();
    let used: Vec<&str> = bridge
        .lines()
        .filter_map(|line| line.trim().strip_prefix("use super::"))
        .map(|rest| rest.split([':', ';']).next().unwrap_or_default())
        .collect();
    assert!(!used.is_empty());
    for module in &used {
        assert!(
            ["definitions", "mcp", "wire"].contains(module),
            "the bridge uses {module}"
        );
    }
    assert!(
        !bridge.contains("super::channel"),
        "the bridge names the channel's files"
    );
    for (file, source) in [
        ("bridge.rs", bridge),
        ("definitions.rs", include_str!("definitions.rs")),
        ("mcp.rs", include_str!("mcp.rs")),
        ("wire.rs", include_str!("wire.rs")),
    ] {
        for data in [
            "Database",
            "sqlx",
            "Service",
            "Repository",
            "UseCase",
            "std::fs",
            "File::",
            "crate::context",
            "crate::use_cases",
        ] {
            assert!(!source.contains(data), "{file} names {data}");
        }
    }
}

// The bridge's connector type is usable by a caller outside the module.
#[tokio::test]
async fn a_bridge_reaches_no_application_in_an_empty_folder() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let channel: Option<Box<dyn Channel>> = channel::reach(data_dir.path()).await;
    assert!(channel.is_none());
}
