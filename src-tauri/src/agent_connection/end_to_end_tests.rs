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
    application_with_accounts(data_dir).await.0
}

/// The same application, with the database for a test that breaks it.
async fn application_with_database(
    data_dir: &Path,
) -> (
    Arc<AgentGate>,
    Arc<crate::context::account::AccountService>,
    sqlx::SqlitePool,
) {
    let (gate, accounts) = application_with_accounts(data_dir).await;
    let database = Database::new(data_dir.to_path_buf())
        .await
        .expect("database");
    (gate, accounts, database.pool)
}

/// The same application, with its account service for the test to read what was recorded.
async fn application_with_accounts(
    data_dir: &Path,
) -> (Arc<AgentGate>, Arc<crate::context::account::AccountService>) {
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
    let gate = Arc::new(AgentGate::new(
        data_dir.to_path_buf(),
        Arc::new(AgentConnections::new(|| {})),
        Arc::new(AgentTools::new(
            accounts,
            assets,
            Arc::clone(&container.currency_service),
        )),
    ));
    (gate, container.account_service)
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
    assert!(!channel::entry_path(data_dir.path()).exists());
    assert_eq!(text(&answers(data_dir.path(), &listing).await[0]), NOT_OPEN);

    assert_eq!(gate.allow(true), Ok(()));
    assert_eq!(gate.allow(true), Ok(()));
    // AGT-024 — a second application over the same folder cannot take the channel of the
    // first, and removes nothing when it closes.
    let second = application(data_dir.path()).await;
    assert_eq!(second.allow(true), Err(GateError::Failed));
    drop(second);
    assert!(channel::entry_path(data_dir.path()).exists());
    assert!(channel::reach(data_dir.path()).await.is_some());
    assert!(gate.is_allowed());
    assert!(channel::entry_path(data_dir.path()).exists());

    drop(gate);
    assert!(!channel::entry_path(data_dir.path()).exists());
    let restarted = application(data_dir.path()).await;
    assert!(restarted.is_allowed());
    restarted.open_if_allowed();
    assert!(channel::entry_path(data_dir.path()).exists());

    assert_eq!(restarted.allow(false), Ok(()));
    assert_eq!(restarted.allow(false), Ok(()));
    assert!(!restarted.is_allowed());
    assert!(!channel::entry_path(data_dir.path()).exists());
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

// AGT-044 / AGT-045 — the three recordings go through the window's rules, and each is
// marked with the agent and its session; a transaction the owner typed carries no mark.
#[tokio::test]
async fn agt_044_an_agent_records_through_the_rules_and_its_recordings_are_marked() {
    use crate::context::account::{JournalFilter, TransactionType};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let (gate, accounts) = application_with_accounts(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let owner = owner_answers(&gate, true);

    let answered = answers(
        data_dir.path(),
        &[
            json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
                "params": { "clientInfo": { "name": "claude-code" } } })
            .to_string(),
            call(1, "record_opening_balance", json!({ "account": "PEA", "asset": "CW8",
                "quantity": "10", "total_cost": 4000, "date": "2026-01-05" })),
            call(2, "record_purchase", json!({ "account": "pea", "asset": "cw8",
                "quantity": 2, "price": "100.5", "date": "2026-02-01", "note": "from a statement" })),
            call(3, "record_sale", json!({ "account": "PEA", "asset": "CW8",
                "quantity": "1", "total": "150", "date": "2026-03-01" })),
            call(4, "record_sale", json!({ "account": "PEA", "asset": "CW8",
                "quantity": "500", "price": "1", "date": "2026-03-02" })),
            call(5, "record_purchase", json!({ "account": "PEA", "asset": "NOPE",
                "quantity": "1", "price": "1" })),
            call(6, "record_purchase", json!({ "account": "PEA", "asset": "CW8",
                "quantity": "1", "price": "1", "total": "1" })),
            call(7, "record_purchase", json!({ "account": "PEA", "asset": "CW8",
                "quantity": "1,5", "price": "1" })),
            call(8, "record_opening_balance", json!({ "account": "PEA", "asset": "CW8",
                "quantity": "1" })),
        ],
    )
    .await;
    owner.await.expect("owner");

    let opened = result(&answered[1]);
    assert_eq!(opened["status"], "recorded");
    assert_eq!(opened["transaction"]["transaction_type"], "OpeningBalance");
    assert_eq!(opened["transaction"]["quantity"], 10 * M);
    assert_eq!(opened["transaction"]["total_amount"], 4_000 * M);
    assert_eq!(opened["account"], "PEA");
    let bought = result(&answered[2]);
    assert_eq!(bought["transaction"]["unit_price"], 100_500_000);
    assert_eq!(bought["transaction"]["note"], "from a statement");
    let sold = result(&answered[3]);
    assert_eq!(sold["transaction"]["transaction_type"], "Sell");

    for (index, said) in [
        (5, "no asset matches \"NOPE\" by name, reference or ISIN"),
        (6, "give either price or total, not both"),
        (
            7,
            "quantity is not a decimal with a dot and at most six decimals: \"1,5\"",
        ),
        (8, "total_cost is missing"),
    ] {
        assert_eq!(answered[index]["result"]["isError"], true, "{index}");
        assert_eq!(text(&answered[index]), said);
    }
    assert_eq!(
        answered[4]["result"]["isError"], true,
        "an oversell is refused"
    );

    let account = accounts.get_all().await.expect("accounts").remove(0);
    let journal = accounts
        .get_account_journal(&account.id, &JournalFilter::default())
        .await
        .expect("journal");
    assert_eq!(
        journal.rows.len(),
        4,
        "the deposit and the three recordings"
    );
    let mut sessions = std::collections::HashSet::new();
    for row in &journal.rows {
        match (&row.transaction.transaction_type, &row.recorded_by) {
            (TransactionType::Deposit, mark) => assert!(mark.is_none(), "typed by the owner"),
            (_, Some(mark)) => {
                assert_eq!(mark.agent, "claude-code");
                assert!(!mark.session_started_at.is_empty());
                sessions.insert(mark.session.clone());
            }
            (kind, None) => panic!("{kind} is not marked"),
        }
    }
    assert_eq!(sessions.len(), 1, "one session recorded all three");

    // AGT-045 — the mark goes with its transaction: a correction keeps it, a removal
    // removes it.
    let sale = journal
        .rows
        .iter()
        .find(|row| row.transaction.transaction_type == TransactionType::Sell)
        .expect("the sale")
        .transaction
        .clone();
    accounts
        .correct_transaction(
            &account.id,
            &sale.id,
            sale.date.clone(),
            sale.quantity,
            sale.unit_price,
            sale.exchange_rate,
            2 * M,
            None,
            Some("corrected by the owner".to_string()),
        )
        .await
        .expect("corrected");
    let marked_after = |journal: &crate::context::account::AccountJournal| -> usize {
        journal
            .rows
            .iter()
            .filter(|row| row.recorded_by.is_some())
            .count()
    };
    let corrected = accounts
        .get_account_journal(&account.id, &JournalFilter::default())
        .await
        .expect("journal");
    assert_eq!(marked_after(&corrected), 3, "a correction keeps the mark");

    accounts
        .cancel_transaction(&account.id, &sale.id)
        .await
        .expect("cancelled");
    let after_removal = accounts
        .get_account_journal(&account.id, &JournalFilter::default())
        .await
        .expect("journal");
    assert_eq!(after_removal.rows.len(), 3);
    assert_eq!(marked_after(&after_removal), 2, "the mark went with it");
}

// AGT-049 — a recording that cannot be marked is undone: when the mark cannot be
// written the recording is undone, and the agent is told nothing was recorded.
#[tokio::test]
async fn agt_045_a_recording_that_cannot_be_marked_is_undone() {
    use crate::context::account::JournalFilter;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let (gate, accounts, pool) = application_with_database(data_dir.path()).await;
    sqlx::query("DROP TABLE agent_recordings")
        .execute(&pool)
        .await
        .expect("the marks cannot be written");
    gate.allow(true).expect("allowed");
    let owner = owner_answers(&gate, true);

    let answered = answers(
        data_dir.path(),
        &[call(
            1,
            "record_opening_balance",
            json!({ "account": "PEA", "asset": "CW8",
            "quantity": "10", "total_cost": 4000, "date": "2026-01-05" }),
        )],
    )
    .await;
    owner.await.expect("owner");

    assert_eq!(answered[0]["result"]["isError"], true);
    assert_eq!(
        text(&answered[0]),
        "the recording could not be marked as an agent's, so nothing was recorded: try again"
    );
    let account = accounts.get_all().await.expect("accounts").remove(0);
    // The journal reads the marks too: with their table gone it cannot be read, so the
    // transactions are counted directly.
    assert!(accounts
        .get_account_journal(&account.id, &JournalFilter::default())
        .await
        .is_err());
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(left, 1, "only the owner's deposit");
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
            call(
                5,
                "list_holdings",
                json!({ "account": "PEA", "as_of": 20260105 }),
            ),
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
    // A date sent as a number is refused, never read as today.
    assert_eq!(answered[4]["result"]["isError"], true);
    assert_eq!(
        text(&answered[4]),
        "as_of is not a past date written YYYY-MM-DD"
    );
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
    let mut folders = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agent_connection")];
    let mut sources = Vec::new();
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).expect("a folder of the module") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                folders.push(path);
            } else {
                sources.push(path);
            }
        }
    }
    let mut scanned = 0;
    for path in sources {
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
    assert!(scanned >= 12, "every file of the module is scanned");
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

/// The tools over the portfolio of `data_dir`, with what the tests read and the owner uses.
struct Workbench {
    tools: AgentTools,
    connections: Arc<AgentConnections>,
    accounts: Arc<crate::context::account::AccountService>,
    assets: Arc<crate::context::asset::AssetService>,
    recorder: crate::use_cases::holding_transaction::HoldingTransactionUseCase,
}

async fn workbench(data_dir: &Path) -> Workbench {
    // Seeds the account, its cash and the asset.
    drop(application_with_accounts(data_dir).await);
    let database = Database::new(data_dir.to_path_buf())
        .await
        .expect("database");
    let container = AppContainer::for_headless_writes(database.pool);
    let accounts: Arc<dyn AccountServiceContract> = Arc::clone(&container.account_service) as _;
    let assets: Arc<dyn AssetServiceContract> = Arc::clone(&container.asset_service) as _;
    let connections = Arc::new(AgentConnections::new(|| {}));
    connections.admit();
    Workbench {
        tools: AgentTools::new(
            Arc::clone(&accounts),
            Arc::clone(&assets),
            Arc::clone(&container.currency_service),
        ),
        connections,
        accounts: container.account_service,
        assets: container.asset_service,
        recorder: crate::use_cases::holding_transaction::HoldingTransactionUseCase::new(
            accounts, assets,
        ),
    }
}

/// A session the owner allowed.
async fn session(connections: &Arc<AgentConnections>) -> super::connections::Granted {
    let asking = Arc::clone(connections);
    let asked = tokio::spawn(async move { asking.ask("claude-code").await });
    let request = loop {
        if let Some(request) = connections.requests().first().cloned() {
            break request;
        }
        tokio::task::yield_now().await;
    };
    assert!(connections.answer(request.id, true));
    asked.await.expect("asked").expect("granted")
}

/// The id of the transaction a recording returned.
fn recorded_id(recorded: &Value) -> String {
    recorded["transaction"]["id"]
        .as_str()
        .expect("an id")
        .to_string()
}

// AGT-050 / AGT-052 — a session corrects and cancels what it recorded itself, through the
// rules of the window, and the window is told what the session did.
#[tokio::test]
async fn agt_050_a_session_corrects_and_cancels_what_it_recorded() {
    use crate::context::account::TransactionType;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    call("list_accounts", json!({})).await.expect("listed");
    let purchase = call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 2, "price": 100, "date": "2026-02-01" }),
    )
    .await
    .expect("recorded");
    let id = recorded_id(&purchase);
    let told = bench.connections.sessions().remove(0);
    assert_eq!((told.reads, told.recordings), (1, 1));
    let last = told.last_recording.expect("a last recording");
    assert_eq!(last.kind, TransactionType::Purchase);
    assert_eq!(
        (last.asset.as_str(), last.date.as_str()),
        ("CW8", "2026-02-01")
    );
    assert_eq!(told.started_at, granted.started_at);

    // A new quantity makes the total follow the unit price.
    let more = call(
        "correct_recording",
        json!({ "transaction": id, "quantity": 3 }),
    )
    .await
    .expect("corrected");
    assert_eq!(more["status"], "corrected");
    assert_eq!(more["transaction"]["quantity"], 3 * M);
    assert_eq!(more["transaction"]["total_amount"], 300 * M);
    // A new date alone keeps everything else.
    let moved = call(
        "correct_recording",
        json!({ "transaction": id, "date": "2026-02-03", "note": "the right day" }),
    )
    .await
    .expect("corrected");
    assert_eq!(moved["transaction"]["date"], "2026-02-03");
    assert_eq!(moved["transaction"]["quantity"], 3 * M);
    assert_eq!(moved["transaction"]["total_amount"], 300 * M);
    assert_eq!(moved["transaction"]["note"], "the right day");
    // A new total is taken as typed.
    let priced = call(
        "correct_recording",
        json!({ "transaction": id, "total": "330" }),
    )
    .await
    .expect("corrected");
    assert_eq!(priced["transaction"]["total_amount"], 330 * M);
    assert_eq!(priced["transaction"]["unit_price"], 110 * M);

    for (arguments, code) in [
        (json!({ "transaction": id }), "MissingArgument"),
        (
            json!({ "transaction": id, "price": 1, "total": 1 }),
            "InvalidArguments",
        ),
        (json!({ "quantity": 1 }), "MissingArgument"),
        (
            json!({ "transaction": id, "quantity": 0 }),
            "QuantityNotPositive",
        ),
    ] {
        let refused = call("correct_recording", arguments)
            .await
            .expect_err("refused");
        assert_eq!(refused.code, code);
    }

    // An opening balance takes a quantity, a total and a date only; its cost stays when its
    // quantity changes.
    let opening = recorded_id(
        &call(
            "record_opening_balance",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "total_cost": 1000, "date": "2026-01-05" }),
        )
        .await
        .expect("recorded"),
    );
    let refused = call(
        "correct_recording",
        json!({ "transaction": opening, "price": 5 }),
    )
    .await
    .expect_err("refused");
    assert_eq!(refused.code, "InvalidArguments");
    let fewer = call(
        "correct_recording",
        json!({ "transaction": opening, "quantity": 8 }),
    )
    .await
    .expect("corrected");
    assert_eq!(fewer["transaction"]["quantity"], 8 * M);
    assert_eq!(fewer["transaction"]["total_amount"], 1000 * M);
    call("cancel_recording", json!({ "transaction": opening }))
        .await
        .expect("cancelled");

    let cancelled = call("cancel_recording", json!({ "transaction": id }))
        .await
        .expect("cancelled");
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["transaction"], id);
    assert!(bench
        .accounts
        .get_transaction_by_id(&id)
        .await
        .expect("read")
        .is_none());
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 0);
    assert_eq!(told.last_recording, None);
}

// AGT-060 / AGT-061 — a session records a deposit, a withdrawal and a dividend through the
// rules of the window: each is marked and counted, cancelled like any recording, never
// corrected.
#[tokio::test]
async fn agt_060_a_session_records_cash_and_a_dividend() {
    use crate::context::account::TransactionType;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    let deposit = call(
        "record_deposit",
        json!({ "account": "pea", "amount": "1500.5", "date": "2019-01-15", "note": "statement 2019" }),
    )
    .await
    .expect("deposited");
    assert_eq!(deposit["status"], "recorded");
    assert_eq!(deposit["transaction"]["transaction_type"], "Deposit");
    assert_eq!(deposit["transaction"]["total_amount"], 1_500_500_000_i64);
    assert_eq!(deposit["transaction"]["note"], "statement 2019");
    assert_eq!(deposit["account"], "PEA");
    assert_eq!(deposit["asset"], Value::Null);

    // A withdrawal above the cash the account held on that date is the window's refusal.
    let overdrawn = call(
        "record_withdrawal",
        json!({ "account": "PEA", "amount": 2000, "date": "2019-02-01" }),
    )
    .await
    .expect_err("refused");
    assert_eq!(overdrawn.code, "InsufficientCash");
    let withdrawal = call(
        "record_withdrawal",
        json!({ "account": "PEA", "amount": 500, "date": "2019-02-01" }),
    )
    .await
    .expect("withdrawn");
    assert_eq!(withdrawal["transaction"]["transaction_type"], "Withdrawal");

    // A dividend of an asset the account does not hold is refused; held, it is recorded
    // in the account's currency at the rate given.
    let not_held = call(
        "record_dividend",
        json!({ "account": "PEA", "asset": "CW8", "amount": 10, "date": "2019-03-01" }),
    )
    .await
    .expect_err("refused");
    assert_eq!(not_held.code, "AssetNotHeld");
    call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 2, "price": 100, "date": "2019-03-01" }),
    )
    .await
    .expect("bought");
    let dividend = call(
        "record_dividend",
        json!({ "account": "PEA", "asset": "cw8", "amount": "10", "rate": 0.5, "date": "2019-06-01" }),
    )
    .await
    .expect("paid");
    assert_eq!(dividend["transaction"]["transaction_type"], "Dividend");
    assert_eq!(dividend["transaction"]["total_amount"], 5 * M);
    assert_eq!(dividend["asset"], "CW8");

    // What is missing or misread is refused as for any recording (AGT-048).
    for (tool, arguments, code) in [
        (
            "record_deposit",
            json!({ "account": "PEA" }),
            "MissingArgument",
        ),
        ("record_deposit", json!({ "amount": 1 }), "MissingArgument"),
        (
            "record_deposit",
            json!({ "account": "PEA", "amount": "1,5" }),
            "InvalidFigure",
        ),
        (
            "record_deposit",
            json!({ "account": "PEA", "amount": 0 }),
            "AmountNotPositive",
        ),
        (
            "record_withdrawal",
            json!({ "account": "NOPE", "amount": 1 }),
            "AccountNotFound",
        ),
        (
            "record_dividend",
            json!({ "account": "PEA", "amount": 1 }),
            "MissingArgument",
        ),
        (
            "record_dividend",
            json!({ "account": "PEA", "asset": "NOPE", "amount": 1 }),
            "AssetNotFound",
        ),
        (
            "record_deposit",
            json!({ "account": "PEA", "amount": 1, "date": 20190101 }),
            "InvalidArguments",
        ),
    ] {
        assert_eq!(
            call(tool, arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{tool} {arguments}"
        );
    }

    // Each is marked with the session and counted; the last one is what the window names.
    for recorded in [&deposit, &withdrawal, &dividend] {
        let mark = bench
            .accounts
            .agent_recording_of(&recorded_id(recorded))
            .await
            .expect("read")
            .expect("a mark");
        assert_eq!(mark.session, granted.key);
        assert_eq!(mark.agent, "claude-code");
    }
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 4);
    let last = told.last_recording.expect("a last recording");
    assert_eq!(last.kind, TransactionType::Dividend);
    assert_eq!(
        (last.asset.as_str(), last.date.as_str()),
        ("CW8", "2019-06-01")
    );

    // Never corrected: cancelled and recorded again.
    for recorded in [&deposit, &withdrawal, &dividend] {
        let refusal = call(
            "correct_recording",
            json!({ "transaction": recorded_id(recorded), "date": "2019-07-01" }),
        )
        .await
        .expect_err("refused");
        assert_eq!(refusal.code, "InvalidArguments");
        assert_eq!(
            refusal.message,
            "a deposit, a withdrawal, a dividend, a split or free shares is not corrected: cancel it and record it again"
        );
    }
    let cancelled = call(
        "cancel_recording",
        json!({ "transaction": recorded_id(&dividend) }),
    )
    .await
    .expect("cancelled");
    assert_eq!(cancelled["status"], "cancelled");
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 3);
    // The purchase is the last recorded that is left; a deposit names the Cash Asset.
    call(
        "cancel_recording",
        json!({ "transaction": recorded_id(&withdrawal) }),
    )
    .await
    .expect("cancelled");
    let deposit_again = call("record_deposit", json!({ "account": "PEA", "amount": 1 }))
        .await
        .expect("deposited");
    assert_eq!(deposit_again["asset"], Value::Null);
    let last = bench
        .connections
        .sessions()
        .remove(0)
        .last_recording
        .expect("a last recording");
    assert_eq!(last.kind, TransactionType::Deposit);
    assert_eq!(last.asset, "EUR");
}

// AGT-062 — a session records a price: stored with the source Agent, replacing the price
// at that date like any price, counted against the session's limit and not among its
// transactions.
#[tokio::test]
async fn agt_062_a_session_records_a_price() {
    use crate::context::asset::AssetPriceSource;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    let recorded = call(
        "record_price",
        json!({ "asset": "cw8", "price": "512.30", "date": "2019-12-31" }),
    )
    .await
    .expect("recorded");
    assert_eq!(recorded["status"], "recorded");
    assert_eq!(recorded["price"]["price"], 512_300_000);
    assert_eq!(recorded["price"]["source"], "Agent");
    assert_eq!(recorded["price"]["date"], "2019-12-31");
    assert_eq!(recorded["asset"], "CW8");
    assert_eq!(recorded["currency"], "EUR");

    // The latest write wins, as for every price: an agent's price replaces its own and one
    // the owner typed, and the row then reads Agent.
    let asset = bench
        .assets
        .get_non_cash_assets()
        .await
        .expect("assets")
        .remove(0);
    bench
        .assets
        .record_asset_price_in_micros(&asset.id, "2020-12-31", 700 * M)
        .await
        .expect("typed");
    for (date, price) in [("2019-12-31", 600), ("2020-12-31", 710)] {
        let replaced = call(
            "record_price",
            json!({ "asset": "CW8", "price": price, "date": date }),
        )
        .await
        .expect("recorded");
        assert_eq!(replaced["price"]["source"], "Agent");
    }
    let prices = bench
        .assets
        .get_asset_prices(&asset.id)
        .await
        .expect("prices");
    let stored: Vec<(&str, i64, &AssetPriceSource)> = prices
        .iter()
        .map(|price| (price.date.as_str(), price.price, &price.source))
        .collect();
    assert_eq!(
        stored,
        vec![
            ("2020-12-31", 710 * M, &AssetPriceSource::Agent),
            ("2019-12-31", 600 * M, &AssetPriceSource::Agent),
        ]
    );
    // The owner's next write wins in turn.
    bench
        .assets
        .record_asset_price_in_micros(&asset.id, "2020-12-31", 705 * M)
        .await
        .expect("typed again");
    let latest = bench
        .assets
        .get_asset_prices(&asset.id)
        .await
        .expect("prices")
        .remove(0);
    assert_eq!(
        (latest.price, latest.source),
        (705 * M, AssetPriceSource::Manual)
    );

    for (arguments, code) in [
        (json!({ "asset": "CW8" }), "MissingArgument"),
        (json!({ "price": 1 }), "MissingArgument"),
        (json!({ "asset": "CW8", "price": "1,5" }), "InvalidFigure"),
        (
            json!({ "asset": "CW8", "price": 0, "date": "2018-12-31" }),
            "NotPositive",
        ),
        (
            json!({ "asset": "CW8", "price": 1, "date": "2999-01-01" }),
            "DateInFuture",
        ),
        (
            json!({ "asset": "CW8", "price": 1, "date": 20181231 }),
            "InvalidArguments",
        ),
        (json!({ "asset": "NOPE", "price": 1 }), "AssetNotFound"),
    ] {
        assert_eq!(
            call("record_price", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }

    // A price is no transaction: the session's transactions are as they were.
    assert_eq!(bench.connections.sessions().remove(0).recordings, 0);
    // Its three prices took three places of the session's limit: 497 more and the next is
    // refused.
    for _ in 3..super::connections::MAX_RECORDINGS {
        granted.count_recording();
    }
    let past_the_limit = call(
        "record_price",
        json!({ "asset": "CW8", "price": 1, "date": "2018-12-31" }),
    )
    .await
    .expect_err("refused");
    assert_eq!(past_the_limit.code, "SessionLimitReached");
}

// AGT-051 — a transaction the owner typed, one another session recorded and one that does
// not exist are refused alike, in the same words: nothing is said of what exists.
#[tokio::test]
async fn agt_051_a_session_touches_nothing_it_did_not_record() {
    use crate::context::account::JournalFilter;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let first = session(&bench.connections).await;
    let second = session(&bench.connections).await;
    let of_the_first = recorded_id(
        &bench
            .tools
            .call(
                &first,
                "record_purchase",
                &json!({ "account": "PEA", "asset": "CW8", "quantity": 2, "price": 100, "date": "2026-02-01" }),
            )
            .await
            .expect("recorded"),
    );
    let account = bench.accounts.get_all().await.expect("accounts").remove(0);
    let journal = bench
        .accounts
        .get_account_journal(&account.id, &JournalFilter::default())
        .await
        .expect("journal");
    let typed_by_the_owner = journal
        .rows
        .iter()
        .find(|row| row.recorded_by.is_none())
        .expect("the owner's deposit")
        .transaction
        .id
        .clone();

    let mut said = std::collections::HashSet::new();
    for transaction in [
        typed_by_the_owner.as_str(),
        of_the_first.as_str(),
        "no-such-transaction",
    ] {
        for (tool, arguments) in [
            ("cancel_recording", json!({ "transaction": transaction })),
            (
                "correct_recording",
                json!({ "transaction": transaction, "quantity": 1 }),
            ),
        ] {
            let refused = bench
                .tools
                .call(&second, tool, &arguments)
                .await
                .expect_err("refused");
            assert_eq!(refused.code, "NotYourRecording", "{tool} {transaction}");
            said.insert(refused.message);
        }
    }
    assert_eq!(said.len(), 1, "one sentence for every case");
    let after = bench
        .accounts
        .get_account_journal(&account.id, &JournalFilter::default())
        .await
        .expect("journal");
    assert_eq!(after.rows.len(), journal.rows.len(), "nothing was touched");
    assert_eq!(bench.connections.sessions()[0].recordings, 1);
}

// AGT-053 — the owner removes everything a session recorded, in an order its own
// recordings allow, and nothing the owner typed; a recording a later transaction of the
// owner depends on is kept and counted.
#[tokio::test]
async fn agt_053_the_owner_removes_everything_a_session_recorded() {
    use crate::context::account::JournalFilter;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let account = bench.accounts.get_all().await.expect("accounts").remove(0);
    let rows = || async {
        bench
            .accounts
            .get_account_journal(&account.id, &JournalFilter::default())
            .await
            .expect("journal")
            .rows
    };
    let typed = rows().await.len();
    for (tool, arguments) in [
        (
            "record_opening_balance",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "total_cost": 1000, "date": "2026-01-05" }),
        ),
        (
            "record_sale",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 120, "date": "2026-02-01" }),
        ),
        (
            "record_purchase",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 4, "price": 100, "date": "2026-03-01" }),
        ),
    ] {
        bench
            .tools
            .call(&granted, tool, &arguments)
            .await
            .expect("recorded");
    }
    let key = bench
        .connections
        .key_of(granted.session_id)
        .expect("an open session");
    assert_eq!(key, granted.key);
    // The owner sells part of what the session's last purchase brought.
    let asset_id = rows()
        .await
        .iter()
        .find(|row| row.recorded_by.is_some())
        .expect("a recording")
        .transaction
        .asset_id
        .clone();
    bench
        .accounts
        .sell_holding(
            &account.id,
            asset_id,
            "2026-04-01".to_string(),
            2 * M,
            130 * M,
            M,
            0,
            None,
            None,
        )
        .await
        .expect("the owner's sale");

    let removal = bench
        .recorder
        .remove_recorded_in_session(&key)
        .await
        .expect("removed");

    assert_eq!((removal.removed, removal.kept), (2, 1));
    let left = rows().await;
    assert_eq!(
        left.len(),
        typed + 2,
        "what the owner typed, and the purchase kept"
    );
    assert_eq!(
        left.iter().filter(|row| row.recorded_by.is_some()).count(),
        1,
        "the purchase the owner's sale depends on"
    );
    let left_of_it = bench
        .recorder
        .recorded_in_session(&key)
        .await
        .expect("counted");
    assert_eq!(left_of_it.count, 1);
    bench.connections.recordings_are(
        granted.session_id,
        left_of_it.count,
        left_of_it.last.map(super::tools::last_recording),
    );
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 1);
    assert_eq!(
        told.last_recording.map(|last| last.date),
        Some("2026-03-01".to_string()),
        "the last recording named is one that is left"
    );
    assert_eq!(bench.connections.key_of(granted.session_id + 100), None);
}

// AGT-063 — a session records a split and free shares through the window's rules (SPL,
// FSD): each is marked, counted against the session's limit, told to the window,
// cancelled like any recording and never corrected.
#[tokio::test]
async fn agt_063_a_session_records_a_split_and_free_shares() {
    use crate::context::account::TransactionType;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    call(
        "record_deposit",
        json!({ "account": "PEA", "amount": 2000, "date": "2019-01-15" }),
    )
    .await
    .expect("deposited");
    // Neither applies to an asset the account does not hold.
    for (tool, arguments) in [
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 2, "old": 1, "date": "2019-06-01" }),
        ),
        (
            "record_free_shares",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "date": "2019-06-01" }),
        ),
    ] {
        assert_eq!(
            call(tool, arguments).await.expect_err("refused").code,
            "AssetNotHeld",
            "{tool}"
        );
    }
    call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2019-03-01" }),
    )
    .await
    .expect("bought");

    // A split of 2 for 1: the factor is stored, no money moves, and the 10 held are 20 —
    // a sale of 15 after it is no oversell.
    let split = call(
        "record_split",
        json!({ "account": "pea", "asset": "cw8", "new": 2, "old": "1", "date": "2019-06-01", "note": "statement 2019" }),
    )
    .await
    .expect("split");
    assert_eq!(split["status"], "recorded");
    assert_eq!(split["transaction"]["transaction_type"], "Split");
    assert_eq!(split["transaction"]["quantity"], 2 * M);
    assert_eq!(split["transaction"]["total_amount"], 0);
    assert_eq!(split["transaction"]["note"], "statement 2019");
    assert_eq!(split["account"], "PEA");
    assert_eq!(split["asset"], "CW8");
    call(
        "record_sale",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 15, "price": 60, "date": "2019-07-01" }),
    )
    .await
    .expect("sold what the split gave");

    // Free shares: a quantity, fractions included, and no money.
    let free_shares = call(
        "record_free_shares",
        json!({ "account": "PEA", "asset": "CW8", "quantity": "1.5", "date": "2019-08-01" }),
    )
    .await
    .expect("free shares");
    assert_eq!(free_shares["transaction"]["transaction_type"], "FreeShares");
    assert_eq!(free_shares["transaction"]["quantity"], 1_500_000_i64);
    assert_eq!(free_shares["transaction"]["total_amount"], 0);
    assert_eq!(free_shares["asset"], "CW8");

    // What is missing or misread is refused as for any recording (AGT-048); the rest is
    // the window's refusal, with its code.
    for (tool, arguments, code) in [
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 2 }),
            "MissingArgument",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "new": 2, "old": 1 }),
            "MissingArgument",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": "1.5", "old": 1 }),
            "InvalidFigure",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 2.0, "old": 1 }),
            "InvalidFigure",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": true, "old": 1 }),
            "InvalidFigure",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 0, "old": 1 }),
            "SplitFactorNotPositive",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": -2, "old": 1 }),
            "SplitFactorNotPositive",
        ),
        // A ratio whose factor rounds to nothing is no factor (SPL-062).
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 1, "old": 3000000 }),
            "SplitFactorNotPositive",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 3, "old": 3 }),
            "SplitFactorIsOne",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "NOPE", "new": 2, "old": 1 }),
            "AssetNotFound",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 2, "old": 1, "date": 20190101 }),
            "InvalidArguments",
        ),
        (
            "record_free_shares",
            json!({ "account": "PEA", "asset": "CW8" }),
            "MissingArgument",
        ),
        (
            "record_free_shares",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 0 }),
            "QuantityNotPositive",
        ),
        (
            "record_free_shares",
            json!({ "account": "NOPE", "asset": "CW8", "quantity": 1 }),
            "AccountNotFound",
        ),
        (
            "record_free_shares",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "date": "2999-01-01" }),
            "DateInFuture",
        ),
    ] {
        assert_eq!(
            call(tool, arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{tool} {arguments}"
        );
    }

    // Each is marked with the session; the last one is what the window names.
    for recorded in [&split, &free_shares] {
        let mark = bench
            .accounts
            .agent_recording_of(&recorded_id(recorded))
            .await
            .expect("read")
            .expect("a mark");
        assert_eq!(mark.session, granted.key);
        assert_eq!(mark.agent, "claude-code");
    }
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 5);
    let last = told.last_recording.expect("a last recording");
    assert_eq!(last.kind, TransactionType::FreeShares);
    assert_eq!(
        (last.asset.as_str(), last.date.as_str()),
        ("CW8", "2019-08-01")
    );

    // Never corrected: cancelled and recorded again (AGT-061).
    for recorded in [&split, &free_shares] {
        let refusal = call(
            "correct_recording",
            json!({ "transaction": recorded_id(recorded), "date": "2019-08-15" }),
        )
        .await
        .expect_err("refused");
        assert_eq!(refusal.code, "InvalidArguments");
        assert_eq!(
            refusal.message,
            "a deposit, a withdrawal, a dividend, a split or free shares is not corrected: cancel it and record it again"
        );
    }
    let cancelled = call(
        "cancel_recording",
        json!({ "transaction": recorded_id(&free_shares) }),
    )
    .await
    .expect("cancelled");
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(bench.connections.sessions().remove(0).recordings, 4);
    // A split a later sale depends on is the window's refusal to delete.
    assert_eq!(
        call(
            "cancel_recording",
            json!({ "transaction": recorded_id(&split) }),
        )
        .await
        .expect_err("refused")
        .code,
        "CascadingOversell"
    );

    // The five recordings took five places of the session's limit, the split and the
    // free shares among them: with all but one of the others taken, one more is recorded
    // and the next is refused.
    for _ in 5..super::connections::MAX_RECORDINGS - 1 {
        granted.count_recording();
    }
    call(
        "record_free_shares",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "date": "2019-09-01" }),
    )
    .await
    .expect("the last place");
    let past_the_limit = call(
        "record_split",
        json!({ "account": "PEA", "asset": "CW8", "new": 3, "old": 1, "date": "2019-10-01" }),
    )
    .await
    .expect_err("refused");
    assert_eq!(past_the_limit.code, "SessionLimitReached");
}

// AGT-064 — a session adds the assets a statement names, of each kind, through the rules
// of the window's form: the same asset is refused twice, each added asset takes a place
// of the session's limit, and the window is told how many and the last.
#[tokio::test]
async fn agt_064_a_session_adds_an_asset_of_each_kind() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    let listed = call(
        "add_asset",
        json!({ "name": "Air Liquide", "reference": "ai", "currency": "EUR", "kind": "listed", "isin": "FR0000120073" }),
    )
    .await
    .expect("a listed asset");
    assert_eq!(listed["status"], "added");
    assert_eq!(listed["asset"]["name"], "Air Liquide");
    assert_eq!(listed["asset"]["reference"], "AI");
    assert_eq!(listed["asset"]["kind"], "Listed");
    assert_eq!(listed["asset"]["class"], "Stocks");
    assert_eq!(listed["asset"]["isin"], "FR0000120073");
    assert_eq!(listed["reference_shared"], false);
    let crypto = call(
        "add_asset",
        json!({ "name": "Bitcoin", "reference": "BTC", "currency": "EUR", "kind": "Crypto" }),
    )
    .await
    .expect("a crypto asset");
    assert_eq!(crypto["asset"]["class"], "DigitalAsset");
    let custom = call(
        "add_asset",
        json!({ "name": "Flat in Lyon", "reference": "LYON", "currency": "EUR", "class": "realestate" }),
    )
    .await
    .expect("a custom asset");
    assert_eq!(custom["asset"]["kind"], "Custom");

    // The window's refusals, with their codes; what is missing or misread as for any tool.
    for (arguments, code) in [
        (
            json!({ "name": "Air Liquide again", "reference": "AIR", "currency": "EUR", "kind": "listed", "isin": "FR0000120073" }),
            "AssetAlreadyExists",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "kind": "listed" }),
            "IsinRequired",
        ),
        (
            json!({ "name": "Ether", "reference": "ETH", "currency": "EUR", "kind": "crypto", "isin": "FR0000121014" }),
            "IsinNotAllowed",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EURO", "kind": "custom" }),
            "InvalidCurrency",
        ),
        (
            json!({ "reference": "MC", "currency": "EUR", "kind": "custom" }),
            "MissingArgument",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EUR" }),
            "MissingArgument",
        ),
        (
            json!({ "name": "Euros", "reference": "EUR", "currency": "EUR", "kind": "cash" }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "class": "shares" }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "kind": "custom", "isin": 12 }),
            "InvalidArguments",
        ),
        // Text the portfolio keeps and the window shows: bounded, and nothing in it that
        // hides or reorders what is read.
        (
            json!({ "name": "L".repeat(101), "reference": "MC", "currency": "EUR", "kind": "custom" }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "LVMH", "reference": "M".repeat(31), "currency": "EUR", "kind": "custom" }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "LVMH\u{202E}HMVL", "reference": "MC", "currency": "EUR", "kind": "custom" }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC\nlast: X", "currency": "EUR", "kind": "custom" }),
            "InvalidArguments",
        ),
    ] {
        assert_eq!(
            call("add_asset", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }

    // The portfolio has them, the window is told how many and the last, and an asset is
    // no transaction.
    let assets = call("list_assets", json!({})).await.expect("listed");
    let references: Vec<&str> = assets
        .as_array()
        .expect("assets")
        .iter()
        .map(|asset| asset["reference"].as_str().expect("a reference"))
        .collect();
    for reference in ["AI", "BTC", "LYON"] {
        assert!(
            references.contains(&reference),
            "{reference} in {references:?}"
        );
    }
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.assets_added, 3);
    let last = told.last_asset.expect("a last asset");
    assert_eq!(
        (last.name.as_str(), last.reference.as_str()),
        ("Flat in Lyon", "LYON")
    );
    assert_eq!(told.recordings, 0);

    // Its three assets took three places of the session's limit.
    for _ in 3..super::connections::MAX_RECORDINGS {
        granted.count_recording();
    }
    let past_the_limit = call(
        "add_asset",
        json!({ "name": "Gold", "reference": "GOLD", "currency": "EUR", "kind": "custom" }),
    )
    .await
    .expect_err("refused");
    assert_eq!(past_the_limit.code, "SessionLimitReached");
}
