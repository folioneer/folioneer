//! The agent connection from end to end: an agent client's messages enter the bridge, cross
//! the channel, wait for the owner in the running application and are answered from a real
//! portfolio.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::context::account::AccountServiceContract;
use crate::context::asset::{AssetClass, AssetServiceContract, CreateAssetDTO};
use crate::context::sync::SqliteChangeRecorder;
use crate::core::event_bus::{Event, EventReceiver, SideEffectEventBus};
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
        let account = AccountCreationUseCase::new(
            Arc::clone(&accounts),
            Arc::clone(&assets),
            Arc::clone(&container.transactions),
        )
        .create("PEA".to_string(), String::new(), "EUR".to_string(), false)
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
    let connections = Arc::new(AgentConnections::new(|| {}));
    let gate = Arc::new(AgentGate::new(
        data_dir.to_path_buf(),
        Arc::clone(&connections),
        Arc::new(AgentTools::new(
            accounts,
            assets,
            Arc::clone(&container.currency_service),
            Arc::clone(&container.transactions),
            connections,
        )),
    ));
    (gate, container.account_service)
}

/// The application as the window runs it: its services and its agent connection publish on
/// one bus, which the test listens to in the window's place.
async fn application_on_bus(data_dir: &Path) -> (Arc<AgentGate>, EventReceiver) {
    let on_bus = owner_on_bus(data_dir).await;
    (on_bus.gate, on_bus.window)
}

/// The application on its bus, with what the owner answers through and its database.
struct OnBus {
    gate: Arc<AgentGate>,
    window: EventReceiver,
    recorder: crate::use_cases::holding_transaction::HoldingTransactionUseCase,
    pool: sqlx::SqlitePool,
}

async fn owner_on_bus(data_dir: &Path) -> OnBus {
    // Seeds the account, its cash and the asset.
    drop(application_with_accounts(data_dir).await);
    let database = Database::new(data_dir.to_path_buf())
        .await
        .expect("database");
    let bus = Arc::new(SideEffectEventBus::new());
    let window = bus.subscribe();
    let pool = database.pool.clone();
    let recorder = Arc::new(SqliteChangeRecorder::new(database.pool.clone()));
    let container =
        AppContainer::build(database.pool, None, None, Some(Arc::clone(&bus)), recorder);
    let connections = Arc::new(AgentConnections::new(move || {
        bus.publish(Event::AgentConnectionChanged);
    }));
    let gate = Arc::new(AgentGate::new(
        data_dir.to_path_buf(),
        Arc::clone(&connections),
        Arc::new(AgentTools::new(
            Arc::clone(&container.account_service) as _,
            Arc::clone(&container.asset_service) as _,
            Arc::clone(&container.currency_service),
            Arc::clone(&container.transactions),
            connections,
        )),
    ));
    OnBus {
        gate,
        window,
        recorder: crate::use_cases::holding_transaction::HoldingTransactionUseCase::new(
            Arc::clone(&container.account_service) as _,
            Arc::clone(&container.asset_service) as _,
        ),
        pool,
    }
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

/// What an agent client is answered when it sends `lines` to the bridge: it keeps its
/// input open until each request among them is answered, as a client does (AGT-085).
async fn answers(data_dir: &Path, lines: &[String]) -> Vec<Value> {
    let requests = lines
        .iter()
        .filter(|line| {
            serde_json::from_str::<Value>(line).is_ok_and(|message| message.get("id").is_some())
        })
        .count();
    let mut client = LiveClient::start(data_dir);
    for line in lines {
        client
            .say(serde_json::from_str(line).expect("a JSON line"))
            .await;
    }
    let mut answered = Vec::with_capacity(requests);
    for _ in 0..requests {
        answered.push(client.heard().await);
    }
    drop(client.writing);
    client.served.await.expect("served").expect("ended");
    answered
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
        json!([{ "name": "PEA", "currency": "EUR", "country": "FR", "wrapper": null }])
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
    // MKT-206 — the judgement of an outdated price reaches the agent with the figures; the
    // seeded asset is custom, never outdated (MKT-205).
    assert_eq!(summary["summaries"][0]["outdated_price_count"], 0);
    assert_eq!(summary["total"]["outdated_price_count"], 0);
    let holdings = result(&answered[4]);
    assert_eq!(holdings["account_name"], "PEA");
    assert_eq!(holdings["total_global_value"], 5_000 * M);
    assert_eq!(holdings["outdated_price_count"], 0);
    assert!(holdings["holdings"]
        .as_array()
        .expect("holdings")
        .iter()
        .all(|holding| holding["price_outdated"] == false));
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
async fn agt_049_a_recording_that_cannot_be_marked_is_undone() {
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
            // AGT-048A — a date longer than any text an agent may send.
            call(
                6,
                "list_holdings",
                json!({ "account": "PEA", "as_of": "2".repeat(201) }),
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
    assert_eq!(
        text(&answered[5]),
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
    /// The window's transfer, which the owner's answer to a replacement records through.
    transfers: crate::use_cases::transfer::TransferUseCase,
    /// The portfolio's database, for a test that makes it unreadable.
    pool: sqlx::SqlitePool,
    /// The units of work of the window's account creation (ADR-006).
    transactions: Arc<dyn crate::shared::infrastructure::uow::TransactionManager>,
}

async fn workbench(data_dir: &Path) -> Workbench {
    // Seeds the account, its cash and the asset.
    drop(application_with_accounts(data_dir).await);
    let database = Database::new(data_dir.to_path_buf())
        .await
        .expect("database");
    let pool = database.pool.clone();
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
            Arc::clone(&container.transactions),
            Arc::clone(&connections),
        ),
        connections,
        accounts: container.account_service,
        assets: container.asset_service,
        transfers: crate::use_cases::transfer::TransferUseCase::new(
            Arc::clone(&accounts),
            Arc::clone(&assets),
            Arc::clone(&container.currency_service),
        ),
        recorder: crate::use_cases::holding_transaction::HoldingTransactionUseCase::new(
            accounts, assets,
        ),
        pool,
        transactions: container.transactions,
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
    // AGT-052 — the last recording is told as it stands after the correction.
    let told = bench.connections.sessions().remove(0);
    let last = told.last_recording.expect("a last recording");
    assert_eq!(last.date, "2026-02-03");
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

// AGT-060 / AGT-050 — a session records a deposit, a withdrawal and a dividend through the
// rules of the window: each is marked and counted, corrected and cancelled like any recording.
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

    // AGT-050 — corrected as the window corrects them (CSH-023, CSH-033, DIV-040): what
    // each takes, with the window's refusals.
    let more = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&deposit), "amount": 1600 }),
    )
    .await
    .expect("corrected");
    assert_eq!(more["status"], "corrected");
    assert_eq!(more["transaction"]["total_amount"], 1600 * M);
    assert_eq!(more["transaction"]["note"], "statement 2019");
    let paid_more = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&dividend), "amount": 12 }),
    )
    .await
    .expect("corrected");
    assert_eq!(paid_more["transaction"]["total_amount"], 6 * M);
    // DIV-040 — the rate changes what the amount comes to in the account's currency.
    let at_par = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&dividend), "rate": 1 }),
    )
    .await
    .expect("corrected");
    assert_eq!(at_par["transaction"]["total_amount"], 12 * M);
    for (recorded, arguments, code) in [
        (&withdrawal, json!({ "amount": 2000 }), "InsufficientCash"),
        (&withdrawal, json!({ "quantity": 1 }), "InvalidArguments"),
        (&deposit, json!({ "rate": 1 }), "InvalidArguments"),
        (&dividend, json!({ "price": 1 }), "InvalidArguments"),
    ] {
        let mut arguments = arguments;
        arguments["transaction"] = json!(recorded_id(recorded));
        let refused = call("correct_recording", arguments)
            .await
            .expect_err("refused");
        assert_eq!(refused.code, code);
    }
    assert_eq!(
        call(
            "correct_recording",
            json!({ "transaction": recorded_id(&withdrawal), "quantity": 1 }),
        )
        .await
        .expect_err("refused")
        .message,
        "this transaction takes date, note, amount and nothing else"
    );
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
    // AGT-047 — the refusal says how the owner lets the agent go on.
    assert_eq!(
        past_the_limit.message,
        "this session has recorded or added 500 transactions, prices and assets, its limit: the owner disconnects and allows a new session to go on"
    );
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

// AGT-065 — a session records a write-off through the window's rules (WOF): at its cost
// without cash, its reason kept, refused as the window refuses it, previewed, marked,
// counted, corrected and cancelled like any recording.
#[tokio::test]
async fn agt_065_a_session_records_a_write_off() {
    use crate::context::account::{TransactionType, WriteOffReason};

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
    // Nothing is held on the date yet.
    assert_eq!(
        call(
            "record_write_off",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "reason": "Dust", "date": "2019-06-01" }),
        )
        .await
        .expect_err("refused")
        .code,
        "ClosedPosition"
    );
    call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2019-03-01" }),
    )
    .await
    .expect("bought");

    // Previewed first: the quantity falls, the average price and the cash stay.
    let previewed = call(
        "preview_recording",
        json!({ "tool": "record_write_off", "arguments": { "account": "PEA", "asset": "CW8", "quantity": "0.5", "reason": "NetworkFee", "date": "2019-06-01" } }),
    )
    .await
    .expect("previewed");
    assert_eq!(previewed["status"], "previewed");
    assert_eq!(previewed["transaction"]["transaction_type"], "WriteOff");
    assert_eq!(previewed["after"]["holdings"][0]["quantity"], 9_500_000_i64);
    assert_eq!(previewed["after"]["holdings"][0]["average_price"], 100 * M);
    assert_eq!(previewed["after"]["cash"], previewed["before"]["cash"]);

    let written_off = call(
        "record_write_off",
        json!({ "account": "pea", "asset": "cw8", "quantity": "0.5", "reason": "NetworkFee", "date": "2019-06-01", "note": "transfer fee" }),
    )
    .await
    .expect("written off");
    assert_eq!(written_off["status"], "recorded");
    assert_eq!(written_off["transaction"]["transaction_type"], "WriteOff");
    assert_eq!(written_off["transaction"]["quantity"], 500_000_i64);
    assert_eq!(written_off["transaction"]["total_amount"], 0);
    assert_eq!(written_off["transaction"]["write_off_reason"], "NetworkFee");
    assert_eq!(written_off["transaction"]["realized_pnl"], -50 * M);
    assert_eq!(written_off["transaction"]["note"], "transfer fee");
    assert_eq!(written_off["account"], "PEA");
    assert_eq!(written_off["asset"], "CW8");

    for (arguments, code) in [
        (
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1 }),
            "MissingArgument",
        ),
        (
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "reason": "Stolen" }),
            "InvalidArguments",
        ),
        (
            json!({ "account": "PEA", "asset": "CW8", "reason": "Dust" }),
            "MissingArgument",
        ),
        (
            json!({ "account": "PEA", "asset": "CW8", "quantity": 0, "reason": "Dust" }),
            "QuantityNotPositive",
        ),
        (
            json!({ "account": "PEA", "asset": "CW8", "quantity": 20, "reason": "Lost", "date": "2019-07-01" }),
            "Oversell",
        ),
        (
            json!({ "account": "PEA", "asset": "NOPE", "quantity": 1, "reason": "Dust" }),
            "AssetNotFound",
        ),
        (
            json!({ "account": "NOPE", "asset": "CW8", "quantity": 1, "reason": "Dust" }),
            "AccountNotFound",
        ),
        (
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "reason": "Dust", "date": "2999-01-01" }),
            "DateInFuture",
        ),
    ] {
        assert_eq!(
            call("record_write_off", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }

    // Marked with the session; the last one is what the window names.
    let mark = bench
        .accounts
        .agent_recording_of(&recorded_id(&written_off))
        .await
        .expect("read")
        .expect("a mark");
    assert_eq!(mark.session, granted.key);
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 3);
    let last = told.last_recording.expect("a last recording");
    assert_eq!(last.kind, TransactionType::WriteOff);

    // Listed by its type, with its reason (AGT-070).
    let listed = call(
        "list_transactions",
        json!({ "account": "PEA", "transaction_type": "writeoff" }),
    )
    .await
    .expect("listed");
    assert_eq!(listed["total"], 1);
    assert_eq!(listed["transactions"][0]["write_off_reason"], "NetworkFee");
    assert_eq!(previewed["transaction"]["write_off_reason"], "NetworkFee");

    // AGT-050 — corrected as the window corrects it (WOF-030): its quantity, its reason,
    // its date and its note; nothing it does not take.
    let corrected = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&written_off), "quantity": 1, "reason": "Dust" }),
    )
    .await
    .expect("corrected");
    assert_eq!(corrected["status"], "corrected");
    assert_eq!(corrected["transaction"]["quantity"], M);
    assert_eq!(corrected["transaction"]["write_off_reason"], "Dust");
    assert_eq!(corrected["transaction"]["note"], "transfer fee");
    let kept = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&written_off), "date": "2019-06-15" }),
    )
    .await
    .expect("corrected");
    assert_eq!(kept["transaction"]["write_off_reason"], "Dust");
    // AGT-073 — previewed with the reason it would carry, nothing written.
    let previewed_reason = call(
        "preview_recording",
        json!({ "tool": "correct_recording", "arguments": { "transaction": recorded_id(&written_off), "reason": "Lost" } }),
    )
    .await
    .expect("previewed");
    assert_eq!(previewed_reason["transaction"]["write_off_reason"], "Lost");
    assert_eq!(previewed_reason["after"]["holdings"][0]["quantity"], 9 * M);
    let stored = bench
        .accounts
        .get_transaction_by_id(&recorded_id(&written_off))
        .await
        .expect("read")
        .expect("stored");
    assert_eq!(stored.write_off_reason, Some(WriteOffReason::Dust));
    for (arguments, code) in [
        (json!({ "reason": "Gift" }), "InvalidArguments"),
        (json!({ "price": 1 }), "InvalidArguments"),
        (json!({ "quantity": 0 }), "QuantityNotPositive"),
    ] {
        let mut arguments = arguments;
        arguments["transaction"] = json!(recorded_id(&written_off));
        let refused = call("correct_recording", arguments)
            .await
            .expect_err("refused");
        assert_eq!(refused.code, code);
    }
    // #607 — refused as its recording is (WOF-010, WOF-012), naming what was held then.
    for (arguments, code, message) in [
        (
            json!({ "quantity": 11 }),
            "Oversell",
            "only 10 held on that date, not 11",
        ),
        (
            json!({ "date": "2019-02-01" }),
            "ClosedPosition",
            "nothing of this asset is held on that date",
        ),
    ] {
        let mut arguments = arguments;
        arguments["transaction"] = json!(recorded_id(&written_off));
        let refused = call("correct_recording", arguments)
            .await
            .expect_err("refused");
        assert_eq!(
            (refused.code.as_str(), refused.message.as_str()),
            (code, message)
        );
    }
    let sold = call(
        "record_sale",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 9, "price": 110, "date": "2019-08-01" }),
    )
    .await
    .expect("sold");
    // A sale's correction too (SEL-030): nine held before it once the write-off is out.
    for (arguments, code) in [
        (json!({ "quantity": 10 }), "Oversell"),
        (json!({ "date": "2019-02-01" }), "ClosedPosition"),
    ] {
        let mut arguments = arguments;
        arguments["transaction"] = json!(recorded_id(&sold));
        assert_eq!(
            call("correct_recording", arguments)
                .await
                .expect_err("refused")
                .code,
            code
        );
    }
    assert_eq!(
        call(
            "correct_recording",
            json!({ "transaction": recorded_id(&written_off), "quantity": 2 }),
        )
        .await
        .expect_err("refused")
        .code,
        "CascadingOversell"
    );
    // A reason is a write-off's only.
    let other = bench
        .accounts
        .recorded_in_session(&granted.key)
        .await
        .expect("read")
        .into_iter()
        .find(|recording| recording.transaction_id != recorded_id(&written_off))
        .expect("another recording");
    assert_eq!(
        call(
            "correct_recording",
            json!({ "transaction": other.transaction_id, "reason": "Dust" }),
        )
        .await
        .expect_err("refused")
        .code,
        "InvalidArguments"
    );
    let cancelled = call(
        "cancel_recording",
        json!({ "transaction": recorded_id(&written_off) }),
    )
    .await
    .expect("cancelled");
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(bench.connections.sessions().remove(0).recordings, 3);

    // An archived asset is not written off.
    let asset = bench
        .assets
        .get_non_cash_assets()
        .await
        .expect("assets")
        .into_iter()
        .find(|asset| asset.reference == "CW8")
        .expect("CW8");
    bench
        .assets
        .archive_asset(&asset.id)
        .await
        .expect("archived");
    assert_eq!(
        call(
            "record_write_off",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "reason": "Dust" }),
        )
        .await
        .expect_err("refused")
        .code,
        "ArchivedAsset"
    );
}

// AGT-066 / AGT-067 — a session records interest and a management fee through the window's
// rules (INT, FEE): interest on a holding or on the cash, a fee by percent or by what is left,
// refused as the window refuses them, a fee the schedule generated for the period refusing
// the agent's, each marked, counted, corrected and cancelled like any recording, not
// previewed.
#[tokio::test]
async fn agt_066_a_session_records_interest_and_a_management_fee() {
    use crate::context::account::{generated_deduction_id, FeeFrequency, TransactionType};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };
    let fund = bench
        .assets
        .create_asset(CreateAssetDTO {
            kind: None,
            name: "Fonds euros".to_string(),
            reference: "FEUR".to_string(),
            isin: None,
            class: AssetClass::Bonds,
            currency: "EUR".to_string(),
            risk_level: 1,
            category_id: "default-uncategorized".to_string(),
            exchange: None,
            interest_bearing: true,
        })
        .await
        .expect("fund");
    call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "FEUR", "quantity": 10, "price": 100, "date": "2026-02-01" }),
    )
    .await
    .expect("bought");

    // Interest on the holding, by percent and by quantity, at no cost (INT-022, INT-024).
    let by_percent = call(
        "record_interest",
        json!({ "account": "pea", "asset": "feur", "percent": "2.5", "date": "2026-03-01", "note": "2025 statement" }),
    )
    .await
    .expect("credited");
    assert_eq!(by_percent["status"], "recorded");
    assert_eq!(by_percent["transaction"]["transaction_type"], "Interest");
    assert_eq!(by_percent["transaction"]["quantity"], 250_000_i64);
    assert_eq!(by_percent["transaction"]["total_amount"], 0);
    assert_eq!(by_percent["asset"], "FEUR");
    call(
        "record_interest",
        json!({ "account": "PEA", "asset": "FEUR", "quantity": 1, "date": "2026-03-02" }),
    )
    .await
    .expect("credited");
    // Without an asset, the account's cash, in its currency (INT-023).
    let on_cash = call(
        "record_interest",
        json!({ "account": "PEA", "quantity": "12.5", "date": "2026-03-03" }),
    )
    .await
    .expect("credited");
    assert_eq!(on_cash["transaction"]["quantity"], 12_500_000_i64);
    assert_eq!(on_cash["asset"], Value::Null);
    assert_eq!(on_cash["transaction"]["asset_id"], "system-cash-eur");

    for (arguments, code) in [
        (
            json!({ "account": "PEA", "asset": "FEUR" }),
            "InterestAmountInvalid",
        ),
        (
            json!({ "account": "PEA", "asset": "FEUR", "percent": 1, "quantity": 1 }),
            "InterestAmountInvalid",
        ),
        (
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1 }),
            "InterestNotEligible",
        ),
        (
            json!({ "account": "PEA", "asset": "FEUR", "percent": 101 }),
            "PercentageAboveHundred",
        ),
        (
            json!({ "account": "PEA", "asset": "FEUR", "percent": 0 }),
            "PercentageNotPositive",
        ),
        (
            json!({ "account": "PEA", "asset": "FEUR", "quantity": 0 }),
            "QuantityNotPositive",
        ),
        (
            json!({ "account": "PEA", "asset": "NOPE", "quantity": 1 }),
            "AssetNotFound",
        ),
    ] {
        assert_eq!(
            call("record_interest", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }
    // The agent reads the reason, not the code (AGT-020): it names the figures to give.
    assert_eq!(
        call(
            "record_interest",
            json!({ "account": "PEA", "asset": "FEUR" })
        )
        .await
        .expect_err("refused")
        .message,
        "give either percent or quantity"
    );

    // A management fee needs the account's fee parameter (FEE-077).
    let fee = |date: &'static str| json!({ "account": "PEA", "asset": "FEUR", "percent": 1, "date": date });
    assert_eq!(
        call("record_management_fee", fee("2026-03-20"))
            .await
            .expect_err("refused")
            .code,
        "ManagementFeesDisabled"
    );
    let account = bench
        .accounts
        .get_all()
        .await
        .expect("accounts")
        .into_iter()
        .find(|account| account.name == "PEA")
        .expect("PEA");
    bench
        .accounts
        .update(
            account.id.clone(),
            account.name.clone(),
            account.bank_name.clone(),
            account.currency.clone(),
            true,
        )
        .await
        .expect("fees enabled");

    // By percent and by what is left; the quantity falls and no cash moves (FEE-022, FEE-028).
    let charged = call("record_management_fee", fee("2026-03-20"))
        .await
        .expect("charged");
    assert_eq!(charged["status"], "recorded");
    assert_eq!(charged["transaction"]["transaction_type"], "ManagementFee");
    assert_eq!(charged["transaction"]["quantity"], 112_500_i64);
    call(
        "record_management_fee",
        json!({ "account": "PEA", "asset": "FEUR", "resulting_quantity": 11, "date": "2026-03-25" }),
    )
    .await
    .expect("charged");
    for (arguments, code) in [
        (
            json!({ "account": "PEA", "asset": "FEUR" }),
            "ManagementFeeAmountInvalid",
        ),
        (
            json!({ "account": "PEA", "asset": "FEUR", "percent": 1, "resulting_quantity": 1 }),
            "ManagementFeeAmountInvalid",
        ),
        (
            json!({ "account": "PEA", "asset": "FEUR", "resulting_quantity": 50, "date": "2026-03-26" }),
            "ResultingQuantityNotBelowHeld",
        ),
        (
            json!({ "account": "PEA", "asset": "CW8", "percent": 1 }),
            "AssetNotHeld",
        ),
        (
            json!({ "account": "PEA", "asset": "FEUR", "percent": 0 }),
            "PercentageNotPositive",
        ),
        // The cash is never named.
        (
            json!({ "account": "PEA", "asset": "EUR", "percent": 1 }),
            "AssetNotFound",
        ),
    ] {
        assert_eq!(
            call("record_management_fee", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }
    assert_eq!(
        call(
            "record_management_fee",
            json!({ "account": "PEA", "asset": "FEUR" })
        )
        .await
        .expect_err("refused")
        .message,
        "give either percent or resulting_quantity"
    );

    // AGT-067 — the schedule's generated fee of April refuses the agent's for April; a
    // one-off fee of March refused nothing, and once the generated fee is deleted, nothing
    // refuses April's.
    bench
        .accounts
        .create_fee_schedule(
            &account.id,
            fund.id.clone(),
            1_200_000,
            FeeFrequency::Monthly,
            "2026-01-01".to_string(),
            None,
        )
        .await
        .expect("schedule");
    let generated = bench
        .accounts
        .record_generated_management_fee(
            &account.id,
            fund.id.clone(),
            "2026-04-30".to_string(),
            1_200_000,
            12,
            generated_deduction_id(&account.id, &fund.id, "2026-04-30"),
        )
        .await
        .expect("generated");
    let refusal = call("record_management_fee", fee("2026-04-10"))
        .await
        .expect_err("refused");
    assert_eq!(refusal.code, "GeneratedFeeExists");
    assert!(
        refusal.message.contains("request_change"),
        "{}",
        refusal.message
    );
    // A date that is no date is the window's refusal, not a generated fee's.
    assert_eq!(
        call(
            "record_management_fee",
            json!({ "account": "PEA", "asset": "FEUR", "percent": 1, "date": "2026-04-31" }),
        )
        .await
        .expect_err("refused")
        .code,
        "InvalidDate"
    );
    call("record_management_fee", fee("2026-03-28"))
        .await
        .expect("another one-off of March");
    bench
        .accounts
        .cancel_transaction(&account.id, &generated.id)
        .await
        .expect("deleted");
    call("record_management_fee", fee("2026-04-10"))
        .await
        .expect("nothing generated for April any more");

    // Marked, counted and told; corrected, not previewed; cancelled (AGT-050, AGT-072).
    let mark = bench
        .accounts
        .agent_recording_of(&recorded_id(&charged))
        .await
        .expect("read")
        .expect("a mark");
    assert_eq!(mark.session, granted.key);
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 8);
    assert_eq!(
        told.last_recording.expect("a last recording").kind,
        TransactionType::ManagementFee
    );
    // Corrected as the window corrects them (INT-040, FEE-063): a quantity, a date, a note.
    let less = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&by_percent), "quantity": "0.5" }),
    )
    .await
    .expect("corrected");
    assert_eq!(less["status"], "corrected");
    assert_eq!(less["transaction"]["quantity"], 500_000_i64);
    assert_eq!(less["transaction"]["note"], "2025 statement");
    let fee_corrected = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&charged), "quantity": "0.2", "note": "again" }),
    )
    .await
    .expect("corrected");
    assert_eq!(fee_corrected["transaction"]["quantity"], 200_000_i64);
    assert_eq!(fee_corrected["transaction"]["note"], "again");
    // AGT-073 — their correction is previewed, nothing written.
    for (recorded, quantity) in [(&by_percent, "0.4"), (&charged, "0.3")] {
        let previewed = call(
            "preview_recording",
            json!({ "tool": "correct_recording", "arguments": { "transaction": recorded_id(recorded), "quantity": quantity } }),
        )
        .await
        .expect("previewed");
        assert_eq!(previewed["status"], "previewed");
    }
    let unchanged = bench
        .accounts
        .get_transaction_by_id(&recorded_id(&by_percent))
        .await
        .expect("read")
        .expect("stored");
    assert_eq!(unchanged.quantity, 500_000);
    // An argument the tool does not know is ignored (AGT-040): a percent changes nothing.
    for (recorded, arguments, code) in [
        (&by_percent, json!({ "percent": 1 }), "MissingArgument"),
        (&by_percent, json!({ "rate": 1 }), "InvalidArguments"),
        (&charged, json!({ "amount": 1 }), "InvalidArguments"),
    ] {
        let mut arguments = arguments;
        arguments["transaction"] = json!(recorded_id(recorded));
        assert_eq!(
            call("correct_recording", arguments)
                .await
                .expect_err("refused")
                .code,
            code
        );
    }
    assert_eq!(
        call(
            "preview_recording",
            json!({ "tool": "record_interest", "arguments": { "account": "PEA", "quantity": 1 } }),
        )
        .await
        .expect_err("refused")
        .code,
        "InvalidArguments"
    );
    let cancelled = call(
        "cancel_recording",
        json!({ "transaction": recorded_id(&on_cash) }),
    )
    .await
    .expect("cancelled");
    assert_eq!(cancelled["status"], "cancelled");
    let cancelled = call(
        "cancel_recording",
        json!({ "transaction": recorded_id(&charged) }),
    )
    .await
    .expect("cancelled");
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(bench.connections.sessions().remove(0).recordings, 6);
}

// AGT-063 — a session records a split and free shares through the window's rules (SPL,
// FSD): each is marked, counted against the session's limit, told to the window,
// corrected and cancelled like any recording.
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

    // AGT-050 — corrected as the window corrects them (SPL-030, FSD-040): a split by its
    // ratio, both parts; free shares by their quantity.
    let three_for_one = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&split), "new": 3, "old": 1 }),
    )
    .await
    .expect("corrected");
    assert_eq!(three_for_one["status"], "corrected");
    assert_eq!(three_for_one["transaction"]["quantity"], 3 * M);
    assert_eq!(three_for_one["transaction"]["note"], "statement 2019");
    let two = call(
        "correct_recording",
        json!({ "transaction": recorded_id(&free_shares), "quantity": 2 }),
    )
    .await
    .expect("corrected");
    assert_eq!(two["transaction"]["quantity"], 2 * M);
    for (recorded, arguments, code) in [
        (&split, json!({ "new": 4 }), "InvalidArguments"),
        (&split, json!({ "quantity": 4 }), "InvalidArguments"),
        // Ten shares become five: the sale of fifteen is left short.
        (&split, json!({ "new": 1, "old": 2 }), "CascadingOversell"),
        (
            &split,
            json!({ "new": 0, "old": 1 }),
            "SplitFactorNotPositive",
        ),
        (&free_shares, json!({ "price": 1 }), "InvalidArguments"),
    ] {
        let mut arguments = arguments;
        arguments["transaction"] = json!(recorded_id(recorded));
        let refused = call("correct_recording", arguments)
            .await
            .expect_err("refused");
        assert_eq!(refused.code, code);
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

// AGT-068 — a session creates the account a statement names, through the rules of the
// window: usable at once by its name, with its cash line; the same name is refused twice,
// each created account takes a place of the session's limit, and the window is told how
// many and the last.
#[tokio::test]
async fn agt_068_a_session_creates_an_account() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    let created = call(
        "add_account",
        json!({ "name": "  Hardware wallet ", "currency": "EUR", "bank": "Ledger", "country": "DE", "wrapper": "brokerage" }),
    )
    .await
    .expect("an account");
    assert_eq!(created["status"], "created");
    assert_eq!(
        created["account"],
        json!({ "name": "Hardware wallet", "currency": "EUR", "country": "DE", "wrapper": "brokerage" })
    );
    let plain = call(
        "add_account",
        json!({ "name": "Livret", "currency": "USD" }),
    )
    .await
    .expect("an account with only a name and a currency");
    assert_eq!(plain["account"]["wrapper"], Value::Null);

    // Usable at once by its name, with its cash line, its fees off and its bank kept.
    let holdings = call("list_holdings", json!({ "account": "hardware WALLET" }))
        .await
        .expect("its holdings");
    assert_eq!(holdings["account_name"], "Hardware wallet");
    let account = bench
        .accounts
        .get_all()
        .await
        .expect("accounts")
        .into_iter()
        .find(|account| account.name == "Hardware wallet")
        .expect("created");
    assert_eq!(account.bank_name, "Ledger");
    assert!(!account.management_fees_enabled);
    assert!(bench
        .accounts
        .get_holdings_for_account(&account.id)
        .await
        .expect("holdings")
        .iter()
        .any(|holding| holding.asset_id.starts_with("system-cash")));

    // The window's refusals, with their codes; what is missing or misread as for any tool.
    for (arguments, code) in [
        (
            json!({ "name": "pea", "currency": "EUR" }),
            "NameAlreadyExists",
        ),
        (
            json!({ "name": "Other", "currency": "EURO" }),
            "InvalidCurrency",
        ),
        (
            json!({ "name": "Other", "currency": "EUR", "country": "fr" }),
            "InvalidCountry",
        ),
        (
            json!({ "name": "Other", "currency": "EUR", "wrapper": "fr_nowhere" }),
            "WrapperNotAvailable",
        ),
        (json!({ "currency": "EUR" }), "MissingArgument"),
        (
            json!({ "name": "   ", "currency": "EUR" }),
            "MissingArgument",
        ),
        (json!({ "name": "Other" }), "MissingArgument"),
        (
            json!({ "name": "Other", "currency": "EUR", "bank": 12 }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "Other", "currency": "EUR", "wrapper": true }),
            "InvalidArguments",
        ),
        // Text the portfolio keeps and the window shows: bounded, and nothing in it that
        // hides or reorders what is read.
        (
            json!({ "name": "O".repeat(101), "currency": "EUR" }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "Other", "currency": "EUR", "bank": "B".repeat(101) }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "PEA\u{202E}AEP", "currency": "EUR" }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "Other", "currency": "EUR", "bank": "Bank\nlast: X" }),
            "InvalidArguments",
        ),
    ] {
        assert_eq!(
            call("add_account", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }
    let duplicate = call("add_account", json!({ "name": "PEA", "currency": "EUR" }))
        .await
        .expect_err("refused");
    assert!(
        !duplicate.message.contains("PEA"),
        "AGT-042: the reason names only what was sent: {}",
        duplicate.message
    );

    // The window is told how many and the last; an account is no transaction.
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.accounts_added, 2);
    assert_eq!(told.last_account.as_deref(), Some("Livret"));
    assert_eq!(told.recordings, 0);
    assert_eq!(told.transactions_recorded, 0);

    // A blank bank is none, as in the window's form.
    call(
        "add_account",
        json!({ "name": "Savings", "currency": "EUR", "bank": "  " }),
    )
    .await
    .expect("a blank bank is none");

    // Its three accounts took three places of the session's limit and the refusals none:
    // the last place left is still free.
    for _ in 3..super::connections::MAX_RECORDINGS - 1 {
        granted.count_recording();
    }
    call(
        "add_account",
        json!({ "name": "Last place", "currency": "EUR" }),
    )
    .await
    .expect("the last place");
    let past_the_limit = call("add_account", json!({ "name": "Gold", "currency": "EUR" }))
        .await
        .expect_err("refused");
    assert_eq!(past_the_limit.code, "SessionLimitReached");
}

// AGT-069 — an account an agent created carries the agent and its session in the account
// summaries; the owner's carries none, and deleting the account takes its mark with it.
#[tokio::test]
async fn agt_069_an_account_an_agent_created_is_marked() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    bench
        .tools
        .call(
            &granted,
            "add_account",
            &json!({ "name": "Hardware wallet", "currency": "EUR" }),
        )
        .await
        .expect("an account");

    let container = AppContainer::for_headless_writes(bench.pool.clone());
    let rows = crate::use_cases::account_summary::AccountSummaryUseCase::new(
        Arc::clone(&container.account_service) as _,
        Arc::clone(&container.asset_service) as _,
        Arc::clone(&container.currency_service),
    )
    .get_account_summaries()
    .await
    .expect("the summaries")
    .summaries;
    let created_by = |name: &str| {
        rows.iter()
            .find(|row| row.name == name)
            .map(|row| row.created_by.clone())
            .expect("a row")
    };
    let mark = created_by("Hardware wallet").expect("marked");
    assert_eq!(
        (
            mark.agent.as_str(),
            mark.session.as_str(),
            mark.session_started_at.as_str()
        ),
        (
            granted.client.as_str(),
            granted.key.as_str(),
            granted.started_at.as_str()
        )
    );
    assert_eq!(created_by("PEA"), None);

    // AGT-040 — an agent reads the summaries without the marks: they name session keys.
    let read = bench
        .tools
        .call(&granted, "portfolio_summary", &json!({}))
        .await
        .expect("the summaries");
    assert!(read["summaries"]
        .as_array()
        .expect("rows")
        .iter()
        .all(|row| row.get("created_by").is_none()));

    let account = bench
        .accounts
        .get_all()
        .await
        .expect("accounts")
        .into_iter()
        .find(|account| account.name == "Hardware wallet")
        .expect("created");
    bench.accounts.delete(&account.id).await.expect("deleted");
    let marks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_account_creations")
        .fetch_one(&bench.pool)
        .await
        .expect("counted");
    assert_eq!(marks, 0, "the mark goes with its account");

    // SYN-023 — an account another computer deleted takes its mark with it too.
    bench
        .tools
        .call(
            &granted,
            "add_account",
            &json!({ "name": "Livret", "currency": "EUR" }),
        )
        .await
        .expect("an account");
    let livret = bench
        .accounts
        .get_all()
        .await
        .expect("accounts")
        .into_iter()
        .find(|account| account.name == "Livret")
        .expect("created");
    let mut conn = bench.pool.acquire().await.expect("a connection");
    use crate::context::account::AccountRepository;
    crate::context::account::SqliteAccountRepository::new(bench.pool.clone())
        .remove_account(&mut conn, &livret.id)
        .await
        .expect("removed");
    let marks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM agent_account_creations")
        .fetch_one(&bench.pool)
        .await
        .expect("counted");
    assert_eq!(
        marks, 0,
        "the mark goes with its account, deleted from elsewhere"
    );
}

// AGT-069 — the account and its mark are written together: a mark that cannot be written
// creates nothing, and the creation is refused and takes no place of the limit.
#[tokio::test]
async fn agt_069_an_account_whose_mark_cannot_be_written_is_not_created() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    sqlx::query("DROP TABLE agent_account_creations")
        .execute(&bench.pool)
        .await
        .expect("dropped");

    let refused = bench
        .tools
        .call(
            &granted,
            "add_account",
            &json!({ "name": "Hardware wallet", "currency": "EUR" }),
        )
        .await
        .expect_err("refused");
    assert_eq!(refused.code, "DatabaseError");
    assert!(bench
        .accounts
        .get_all()
        .await
        .expect("accounts")
        .iter()
        .all(|account| account.name != "Hardware wallet"));
    assert_eq!(bench.connections.sessions().remove(0).accounts_added, 0);
}

// AGT-070 — a session lists an account's transactions, oldest first and in pages, with the
// id of each and who recorded it: the owner, an agent, or this session. It is a read.
#[tokio::test]
async fn agt_070_a_session_lists_the_transactions_of_an_account() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let other = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    // The owner typed a deposit on 2026-01-02 (the seed); this session records two
    // transactions and another session one.
    call(
        "record_deposit",
        json!({ "account": "PEA", "amount": 2000, "date": "2019-01-15" }),
    )
    .await
    .expect("deposited");
    let purchase = call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2019-03-01", "note": "statement 2019" }),
    )
    .await
    .expect("bought");
    bench
        .tools
        .call(
            &other,
            "record_deposit",
            &json!({ "account": "PEA", "amount": 50, "date": "2019-05-01" }),
        )
        .await
        .expect("deposited by another session");

    let all = call("list_transactions", json!({ "account": "pea" }))
        .await
        .expect("listed");
    assert_eq!(all["status"], "listed");
    assert_eq!(all["account"], "PEA");
    assert_eq!(all["currency"], "EUR");
    assert_eq!(
        (
            all["total"].clone(),
            all["page"].clone(),
            all["pages"].clone()
        ),
        (json!(4), json!(1), json!(1))
    );
    let rows = all["transactions"].as_array().expect("transactions");
    let seen: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|row| {
            (
                row["date"].as_str().expect("a date"),
                row["transaction_type"].as_str().expect("a type"),
                row["recorded_by"]["by"].as_str().expect("who"),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("2019-01-15", "Deposit", "session"),
            ("2019-03-01", "Purchase", "session"),
            ("2019-05-01", "Deposit", "agent"),
            ("2026-01-02", "Deposit", "owner"),
        ]
    );
    // A row says what a recording's answer says: the id, the figures in millionths, the
    // asset by its reference — none for cash — the note, and the agent that recorded it.
    let bought = &rows[1];
    assert_eq!(bought["id"], purchase["transaction"]["id"]);
    assert_eq!(bought["asset"], "CW8");
    assert_eq!(bought["quantity"], 10 * M);
    assert_eq!(bought["unit_price"], 100 * M);
    assert_eq!(bought["total_amount"], 1_000 * M);
    assert_eq!(bought["fees"], 0);
    assert_eq!(bought["exchange_rate"], M);
    assert_eq!(bought["note"], "statement 2019");
    assert_eq!(bought["recorded_by"]["agent"], "claude-code");
    assert_eq!(rows[0]["asset"], Value::Null);
    assert_eq!(rows[3]["recorded_by"], json!({ "by": "owner" }));

    // Each filter keeps its rows, and they combine.
    for (arguments, dates) in [
        (
            json!({ "account": "PEA", "asset": "cw8" }),
            vec!["2019-03-01"],
        ),
        (
            json!({ "account": "PEA", "from": "2019-02-01", "to": "2019-12-31" }),
            vec!["2019-03-01", "2019-05-01"],
        ),
        (
            json!({ "account": "PEA", "transaction_type": "deposit" }),
            vec!["2019-01-15", "2019-05-01", "2026-01-02"],
        ),
        (
            json!({ "account": "PEA", "recorded_by": "owner" }),
            vec!["2026-01-02"],
        ),
        (
            json!({ "account": "PEA", "recorded_by": "agent" }),
            vec!["2019-01-15", "2019-03-01", "2019-05-01"],
        ),
        (
            json!({ "account": "PEA", "recorded_by": "session" }),
            vec!["2019-01-15", "2019-03-01"],
        ),
        (
            json!({ "account": "PEA", "recorded_by": "session", "transaction_type": "Deposit", "to": "2019-01-15" }),
            vec!["2019-01-15"],
        ),
        (json!({ "account": "PEA", "from": "2030-01-01" }), vec![]),
    ] {
        let listed = call("list_transactions", arguments.clone())
            .await
            .expect("listed");
        let found: Vec<&str> = listed["transactions"]
            .as_array()
            .expect("transactions")
            .iter()
            .map(|row| row["date"].as_str().expect("a date"))
            .collect();
        assert_eq!(found, dates, "{arguments}");
    }

    // What is missing or misread is refused, naming only what was sent (AGT-042).
    for (arguments, code) in [
        (json!({}), "MissingArgument"),
        (json!({ "account": "NOPE" }), "AccountNotFound"),
        (
            json!({ "account": "PEA", "asset": "NOPE" }),
            "AssetNotFound",
        ),
        (
            json!({ "account": "PEA", "from": "january" }),
            "InvalidDate",
        ),
        (json!({ "account": "PEA", "to": 20190101 }), "InvalidDate"),
        (
            json!({ "account": "PEA", "transaction_type": "transfer" }),
            "InvalidArguments",
        ),
        (
            json!({ "account": "PEA", "recorded_by": "me" }),
            "InvalidArguments",
        ),
        (json!({ "account": "PEA", "page": 0 }), "InvalidArguments"),
        (
            json!({ "account": "PEA", "page": "two" }),
            "InvalidArguments",
        ),
    ] {
        assert_eq!(
            call("list_transactions", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }

    // It is a read: counted as one each time it is answered, never as a recording, and the
    // id it gives is the one this session cancels with.
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.reads, 9);
    assert_eq!(told.recordings, 2);
    assert!(granted.may_record_one_more());
    call(
        "cancel_recording",
        json!({ "transaction": bought["id"].clone() }),
    )
    .await
    .expect("cancelled by the listed id");

    // A page holds 100 transactions: one more than a page makes two.
    let account = bench.accounts.get_all().await.expect("accounts").remove(0);
    for day in 1..=98 {
        bench
            .accounts
            .record_deposit(&account.id, format!("2020-01-{:02}", day % 28 + 1), M, None)
            .await
            .expect("deposit");
    }
    let first = call("list_transactions", json!({ "account": "PEA" }))
        .await
        .expect("listed");
    assert_eq!(
        (first["total"].clone(), first["pages"].clone()),
        (json!(101), json!(2))
    );
    assert_eq!(first["transactions"].as_array().expect("rows").len(), 100);
    let second = call("list_transactions", json!({ "account": "PEA", "page": 2 }))
        .await
        .expect("listed");
    let last = second["transactions"].as_array().expect("rows");
    assert_eq!(last.len(), 1);
    assert_eq!(last[0]["date"], "2026-01-02");
    let past = call("list_transactions", json!({ "account": "PEA", "page": 3 }))
        .await
        .expect("listed");
    assert!(past["transactions"].as_array().expect("rows").is_empty());
}

// AGT-071 / AGT-040 / MKT-230/231 (#529) — get_price lists every reported split of the asset,
// whatever the date asked; list_holdings lists a holding's unrecorded splits, and a split
// recorded on that date ends it.
#[tokio::test]
async fn agt_071_an_agent_reads_the_reported_and_the_unrecorded_splits() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };
    let asset = bench
        .assets
        .get_non_cash_assets()
        .await
        .expect("assets")
        .remove(0);
    call(
        "record_opening_balance",
        json!({ "account": "PEA", "asset": "CW8", "quantity": "10", "total_cost": 4000, "date": "2020-01-06" }),
    )
    .await
    .expect("recorded");
    bench
        .assets
        .record_reported_splits(
            &asset.id,
            "2019-01-01",
            &[
                crate::context::asset::DatedSplitFactor {
                    date: "2022-07-18".into(),
                    new: 20,
                    old: 1,
                },
                crate::context::asset::DatedSplitFactor {
                    date: "2024-03-04".into(),
                    new: 1,
                    old: 10,
                },
            ],
        )
        .await
        .expect("kept");

    let read = call("get_price", json!({ "asset": "CW8", "date": "2020-01-06" }))
        .await
        .expect("answered");
    assert_eq!(
        read["reported_splits"],
        json!([
            { "date": "2022-07-18", "new": 20, "old": 1 },
            { "date": "2024-03-04", "new": 1, "old": 10 },
        ])
    );
    assert_eq!(read.as_object().expect("an object").len(), 6);

    let unrecorded = |holdings: &Value| {
        holdings["holdings"]
            .as_array()
            .expect("holdings")
            .iter()
            .find(|holding| holding["asset_reference"] == "CW8")
            .expect("CW8 is held")["unrecorded_splits"]
            .clone()
    };
    let holdings = call("list_holdings", json!({ "account": "PEA" }))
        .await
        .expect("answered");
    assert_eq!(
        unrecorded(&holdings),
        json!([
            { "date": "2022-07-18", "new": 20, "old": 1 },
            { "date": "2024-03-04", "new": 1, "old": 10 },
        ])
    );
    call(
        "record_split",
        json!({ "account": "PEA", "asset": "CW8", "new": 20, "old": 1, "date": "2022-07-18" }),
    )
    .await
    .expect("recorded");
    let holdings = call("list_holdings", json!({ "account": "PEA" }))
        .await
        .expect("answered");
    assert_eq!(
        unrecorded(&holdings),
        json!([{ "date": "2024-03-04", "new": 1, "old": 10 }])
    );
}

// AGT-071 — a session reads the price the application holds for an asset at a date: the
// price of that date or the closest earlier one, with its own date and its source, or
// plainly that there is none. It is a read.
#[tokio::test]
async fn agt_071_a_session_reads_the_price_of_an_asset_at_a_date() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    // An asset with no price says so, and is not refused.
    let none = call("get_price", json!({ "asset": "CW8" }))
        .await
        .expect("answered");
    assert_eq!(none["status"], "no_price");
    assert_eq!(none["price"], Value::Null);
    assert_eq!(none["asset"], "CW8");

    let asset = bench
        .assets
        .get_non_cash_assets()
        .await
        .expect("assets")
        .remove(0);
    bench
        .assets
        .record_asset_price_in_micros(&asset.id, "2019-12-31", 500 * M)
        .await
        .expect("typed by the owner");
    call(
        "record_price",
        json!({ "asset": "CW8", "price": 600, "date": "2020-06-30" }),
    )
    .await
    .expect("recorded by the agent");

    for (asked, found) in [
        (Some("2019-12-31"), Some(("2019-12-31", 500 * M, "Manual"))),
        (Some("2020-03-15"), Some(("2019-12-31", 500 * M, "Manual"))),
        (Some("2020-06-30"), Some(("2020-06-30", 600 * M, "Agent"))),
        (Some("2020-12-31"), Some(("2020-06-30", 600 * M, "Agent"))),
        (None, Some(("2020-06-30", 600 * M, "Agent"))),
        (Some("2019-12-30"), None),
    ] {
        let arguments = match asked {
            Some(date) => json!({ "asset": "cw8", "date": date }),
            None => json!({ "asset": "cw8" }),
        };
        let answer = call("get_price", arguments.clone())
            .await
            .expect("answered");
        assert_eq!(answer["asset"], "CW8", "{arguments}");
        assert_eq!(answer["currency"], "EUR", "{arguments}");
        match found {
            Some((date, price, source)) => {
                assert_eq!(answer["status"], "priced", "{arguments}");
                assert_eq!(answer["price"]["date"], date, "{arguments}");
                assert_eq!(answer["price"]["price"], price, "{arguments}");
                assert_eq!(answer["price"]["source"], source, "{arguments}");
            }
            None => {
                assert_eq!(answer["status"], "no_price", "{arguments}");
                assert_eq!(answer["price"], Value::Null, "{arguments}");
            }
        }
        if let Some(date) = asked {
            assert_eq!(answer["date"], date, "{arguments}");
        }
    }

    // An archived asset keeps its prices, and they are read like any other.
    bench
        .assets
        .archive_asset(&asset.id)
        .await
        .expect("archived");
    let archived = call("get_price", json!({ "asset": "CW8", "date": "2020-01-01" }))
        .await
        .expect("answered");
    assert_eq!(archived["price"]["date"], "2019-12-31");

    for (arguments, code) in [
        (json!({}), "MissingArgument"),
        (json!({ "asset": "NOPE" }), "AssetNotFound"),
        (json!({ "asset": "CW8", "date": "december" }), "InvalidDate"),
        (json!({ "asset": "CW8", "date": 20191231 }), "InvalidDate"),
        (
            json!({ "asset": "CW8", "date": "2999-01-01" }),
            "InvalidDate",
        ),
    ] {
        assert_eq!(
            call("get_price", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }

    // Eight answers, eight reads; the one recorded price is the only place taken.
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.reads, 8);
    for _ in 1..super::connections::MAX_RECORDINGS {
        granted.count_recording();
    }
    assert!(!granted.may_record_one_more());
}

// AGT-036 / B45 / TODO-083 — every write an agent makes reaches the window with the session's
// change after it: an added asset, a transaction on it, a price, an account, then many recordings sent
// at once, none of them lost to the one after.
#[tokio::test]
async fn agt_036_every_write_of_an_agent_reaches_the_window() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let (gate, mut window) = application_on_bus(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let owner = owner_answers(&gate, true);
    const MANY: u32 = 40;
    let mut lines = vec![
        json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": "claude-code" } } })
        .to_string(),
        call(
            1,
            "add_asset",
            json!({ "name": "Air Liquide", "reference": "AI",
            "currency": "EUR", "kind": "custom" }),
        ),
        call(
            2,
            "record_purchase",
            json!({ "account": "PEA", "asset": "AI",
            "quantity": "1", "price": "1", "date": "2026-02-01" }),
        ),
        call(
            3,
            "record_price",
            json!({ "asset": "AI", "price": "2", "date": "2026-02-01" }),
        ),
        call(
            4,
            "add_account",
            json!({ "name": "Hardware wallet", "currency": "EUR" }),
        ),
    ];
    lines.extend((0..MANY).map(|n| {
        call(
            5 + n,
            "record_purchase",
            json!({ "account": "PEA", "asset": "CW8",
            "quantity": "1", "price": "1", "date": "2026-02-02" }),
        )
    }));

    let answered = answers(data_dir.path(), &lines).await;
    owner.await.expect("owner");
    assert!(answered[1..]
        .iter()
        .all(|answer| answer["result"]["isError"] == false));

    // What the window is told, call by call: the session's change closes each call, and a
    // call that wrote is told by its write's event first — a recording's twice, once
    // recorded and once marked (AGT-045).
    let told = window.drain();
    let calls: Vec<&[Event]> = told
        .split(|event| *event == Event::AgentConnectionChanged)
        .collect();
    let mut writes = vec![
        Event::AssetUpdated,
        Event::TransactionUpdated,
        Event::AssetPriceUpdated,
        Event::AccountUpdated,
    ];
    writes.extend((0..MANY).map(|_| Event::TransactionUpdated));
    let mut told_writes = calls.iter().filter(|call| !call.is_empty());
    for write in &writes {
        let call = told_writes
            .next()
            .unwrap_or_else(|| panic!("{write:?} was not told: {told:?}"));
        assert!(
            call.iter().all(|event| event == write),
            "{call:?} for {write:?}"
        );
    }
    assert!(told_writes.next().is_none(), "{told:?}");
    assert_eq!(told.last(), Some(&Event::AgentConnectionChanged));
}

/// CW8 listed in Paris: an asset that shares its reference with the seeded CW8.
fn paris_listing() -> Value {
    json!({ "name": "Amundi MSCI World Paris", "reference": "CW8", "currency": "EUR",
        "kind": "listed", "isin": "LU1681043599", "exchange": "XPAR" })
}

/// The owner answers the next removal request of `connections`.
async fn removal_waiting(connections: &AgentConnections) -> super::connections::RemovalRequest {
    for _ in 0..400 {
        if let Some(request) = connections.removals().into_iter().next() {
            return request;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("no removal request arrived");
}

// AGT-080 / AGT-081 / AGT-082 — an agent asks to remove a transaction the owner typed: the
// window is shown it in full, a refusal gives the agent nothing and removes nothing, the
// next ask is asked again, and a yes removes it through the window's deletion.
#[tokio::test]
async fn agt_080_an_agent_asks_and_the_owner_answers_each_removal() {
    use super::gate::{answer_removal, AnswerRefused};
    use crate::context::account::TransactionType;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let account = bench.accounts.get_all().await.expect("accounts").remove(0);
    let deposit = bench
        .accounts
        .get_account_journal(
            &account.id,
            &crate::context::account::JournalFilter::default(),
        )
        .await
        .expect("journal")
        .rows
        .remove(0)
        .transaction;
    let ask = |granted: super::connections::Granted| {
        let bench = Arc::clone(&bench);
        let id = deposit.id.clone();
        tokio::spawn(async move {
            bench
                .tools
                .call(&granted, "request_removal", &json!({ "transaction": id }))
                .await
        })
    };

    // Asked, and shown in full; the call waits.
    let asked = ask(granted.clone());
    let request = removal_waiting(&bench.connections).await;
    assert_eq!(request.client, "claude-code");
    assert_eq!(request.session, granted.session_id);
    assert_eq!(request.transaction.id, deposit.id);
    assert_eq!(request.transaction.date, "2026-01-02");
    assert_eq!(request.transaction.account, "PEA");
    assert_eq!(request.transaction.currency, "EUR");
    assert_eq!(request.transaction.kind, TransactionType::Deposit);
    assert_eq!(request.transaction.asset_name, None);
    assert_eq!(request.transaction.amount, 5_000 * M);
    assert!(!asked.is_finished());

    // Refused: the agent is given nothing, and nothing is removed.
    answer_removal(&bench.connections, &bench.recorder, request.id, false)
        .await
        .expect("answered");
    let refused = asked.await.expect("asked").expect_err("refused");
    assert_eq!(refused.code, "RemovalRefused");
    assert!(bench.connections.removals().is_empty());
    assert!(bench
        .accounts
        .get_transaction_by_id(&deposit.id)
        .await
        .expect("read")
        .is_some());
    // The request is gone once answered.
    assert_eq!(
        answer_removal(&bench.connections, &bench.recorder, request.id, true).await,
        Err(AnswerRefused::Gone)
    );

    // Nothing of the refusal is kept: the same transaction is asked again, and removed.
    let asked = ask(granted.clone());
    let request = removal_waiting(&bench.connections).await;
    answer_removal(&bench.connections, &bench.recorder, request.id, true)
        .await
        .expect("removed");
    let removed = asked.await.expect("asked").expect("removed");
    assert_eq!(
        removed,
        json!({ "status": "removed", "transaction": deposit.id })
    );
    assert!(bench
        .accounts
        .get_transaction_by_id(&deposit.id)
        .await
        .expect("read")
        .is_none());

    // A transaction an agent recorded is not asked about: the session cancels its own.
    let of_the_agent = recorded_id(
        &bench
            .tools
            .call(
                &granted,
                "record_deposit",
                &json!({ "account": "PEA", "amount": 10, "date": "2026-01-03" }),
            )
            .await
            .expect("recorded"),
    );
    let refused = bench
        .tools
        .call(
            &granted,
            "request_removal",
            &json!({ "transaction": of_the_agent }),
        )
        .await
        .expect_err("refused");
    assert_eq!(refused.code, "NotTypedByOwner");
    assert!(bench.connections.removals().is_empty());
}

// AGT-080 — what is refused before anything is asked, and a deletion the window refuses.
#[tokio::test]
async fn agt_080_a_removal_is_refused_before_asking_or_by_the_deletion() {
    use super::gate::{answer_removal, AnswerRefused};
    use crate::use_cases::holding_transaction::{Recording, Target, Trade, TradeAmount};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    for (arguments, code) in [
        (json!({}), "MissingArgument"),
        (json!({ "transaction": 7 }), "MissingArgument"),
        (
            json!({ "transaction": "no-such-transaction" }),
            "TransactionNotFound",
        ),
        (
            json!({ "transaction": "x".repeat(200) }),
            "TransactionNotFound",
        ),
    ] {
        let refused = bench
            .tools
            .call(&granted, "request_removal", &arguments)
            .await
            .expect_err("refused");
        assert_eq!(refused.code, code, "{arguments}");
    }
    assert!(bench.connections.removals().is_empty(), "nothing was asked");

    // The owner typed an opening balance and a sale that depends on it.
    let target = |date: &str, quantity: i64| Target {
        account: "PEA".to_string(),
        asset: "CW8".to_string(),
        date: Some(date.to_string()),
        quantity,
    };
    let opened = bench
        .recorder
        .record_named(
            Recording::Open {
                target: target("2026-01-05", 10 * M),
                total_cost: 4_000 * M,
            },
            "2026-10-14",
        )
        .await
        .expect("opened");
    bench
        .recorder
        .record_named(
            Recording::Sell(Trade {
                target: target("2026-03-01", 8 * M),
                amount: TradeAmount::Price(500 * M),
                fees: 0,
                rate: M,
                note: None,
            }),
            "2026-10-14",
        )
        .await
        .expect("sold");
    let tools = Arc::clone(&bench);
    let id = opened.transaction.id.clone();
    let waiting_granted = granted.clone();
    let asked = tokio::spawn(async move {
        tools
            .tools
            .call(
                &waiting_granted,
                "request_removal",
                &json!({ "transaction": id }),
            )
            .await
    });
    let request = removal_waiting(&bench.connections).await;
    assert_eq!(
        request.transaction.asset_name.as_deref(),
        Some("Amundi MSCI World")
    );
    assert_eq!(request.transaction.asset_reference.as_deref(), Some("CW8"));
    assert_eq!(request.transaction.quantity, 10 * M);
    let answered = answer_removal(&bench.connections, &bench.recorder, request.id, true).await;
    let Err(AnswerRefused::NotDone { reason }) = answered else {
        panic!("the deletion refused: {answered:?}");
    };
    let refused = asked.await.expect("asked").expect_err("not removed");
    assert_eq!(refused.code, reason);
    assert!(bench
        .accounts
        .get_transaction_by_id(&opened.transaction.id)
        .await
        .expect("read")
        .is_some());

    // AGT-081 — a transaction that changed while its request waited is not removed: the
    // owner answered what was shown.
    let sold = bench
        .accounts
        .get_account_journal(
            &opened.transaction.account_id,
            &crate::context::account::JournalFilter::default(),
        )
        .await
        .expect("journal")
        .rows
        .into_iter()
        .find(|row| {
            row.transaction.transaction_type == crate::context::account::TransactionType::Sell
        })
        .expect("the sale")
        .transaction;
    let ask = |id: String| {
        let tools = Arc::clone(&bench);
        let granted = granted.clone();
        tokio::spawn(async move {
            tools
                .tools
                .call(&granted, "request_removal", &json!({ "transaction": id }))
                .await
        })
    };
    let asked = ask(sold.id.clone());
    let request = removal_waiting(&bench.connections).await;
    bench
        .recorder
        .correct_transaction(
            &sold.account_id,
            &sold.id,
            sold.date.clone(),
            7 * M,
            sold.unit_price,
            sold.exchange_rate,
            sold.fees,
            None,
            None,
        )
        .await
        .expect("corrected in the journal");
    assert_eq!(
        answer_removal(&bench.connections, &bench.recorder, request.id, true).await,
        Err(AnswerRefused::NotDone {
            reason: "TransactionChanged".to_string()
        })
    );
    assert_eq!(
        asked.await.expect("asked").expect_err("changed").code,
        "TransactionChanged"
    );

    // One deleted in the journal while its request waited is not found.
    let asked = ask(sold.id.clone());
    let request = removal_waiting(&bench.connections).await;
    bench
        .recorder
        .cancel_transaction(&sold.account_id, &sold.id)
        .await
        .expect("deleted in the journal");
    assert_eq!(
        answer_removal(&bench.connections, &bench.recorder, request.id, true).await,
        Err(AnswerRefused::NotDone {
            reason: "TransactionNotFound".to_string()
        })
    );
    assert_eq!(
        asked.await.expect("asked").expect_err("gone").code,
        "TransactionNotFound"
    );

    // AGT-082 — a request whose session ends is withdrawn, and its call refused.
    let tools = Arc::clone(&bench);
    let id = opened.transaction.id.clone();
    let waiting_granted = granted.clone();
    let asked = tokio::spawn(async move {
        tools
            .tools
            .call(
                &waiting_granted,
                "request_removal",
                &json!({ "transaction": id }),
            )
            .await
    });
    removal_waiting(&bench.connections).await;
    assert!(bench.connections.end(granted.session_id));
    let refused = asked.await.expect("asked").expect_err("refused");
    assert_eq!(refused.code, "RemovalRefused");
    assert!(bench.connections.removals().is_empty());
}

// AGT-070 — the listing's edges: no page when nothing is kept, bounds the wrong way round
// keep nothing, arguments that are not text, the order within one date, a split's factor
// as its quantity, when the recording session started, and a name that matches two assets.
#[tokio::test]
async fn agt_070_the_edges_of_a_listing() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    let nothing = call(
        "list_transactions",
        json!({ "account": "PEA", "from": "2030-01-01" }),
    )
    .await
    .expect("listed");
    assert_eq!(nothing["total"], 0);
    assert_eq!(nothing["pages"], 0, "{nothing}");
    let reversed = call(
        "list_transactions",
        json!({ "account": "PEA", "from": "2026-12-31", "to": "2019-01-01" }),
    )
    .await
    .expect("listed, not refused");
    assert_eq!(reversed["total"], 0);

    // Three transactions on one date come in the order they were entered; a split's
    // quantity is its factor; a marked one says when its session started.
    for (tool, arguments) in [
        (
            "record_deposit",
            json!({ "account": "PEA", "amount": 2000, "date": "2019-03-01" }),
        ),
        (
            "record_purchase",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2019-03-01" }),
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 3, "old": 1, "date": "2019-03-01" }),
        ),
    ] {
        call(tool, arguments).await.expect("recorded");
    }
    let listed = call(
        "list_transactions",
        json!({ "account": "PEA", "to": "2019-12-31" }),
    )
    .await
    .expect("listed");
    let rows = listed["transactions"].as_array().expect("rows");
    let kinds: Vec<&str> = rows
        .iter()
        .map(|row| row["transaction_type"].as_str().expect("a kind"))
        .collect();
    assert_eq!(kinds, ["Deposit", "Purchase", "Split"]);
    assert_eq!(rows[2]["quantity"], 3 * M);
    assert_eq!(
        rows[0]["recorded_by"]["session_started_at"],
        json!(granted.started_at)
    );

    // An asset whose reference another shares is ambiguous by that reference.
    call("add_asset", paris_listing()).await.expect("added");
    for (arguments, code) in [
        (
            json!({ "account": "PEA", "asset": "CW8" }),
            "AssetAmbiguous",
        ),
        (json!({ "account": "PEA", "page": "2" }), "InvalidArguments"),
        (json!({ "account": 7 }), "MissingArgument"),
        (json!({ "account": "PEA", "asset": 7 }), "InvalidArguments"),
        (
            json!({ "account": "PEA", "transaction_type": 7 }),
            "InvalidArguments",
        ),
        (
            json!({ "account": "PEA", "recorded_by": true }),
            "InvalidArguments",
        ),
    ] {
        assert_eq!(
            call("list_transactions", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }
}

// AGT-071 — the date answered when none is asked is today; a fetched price names its
// provider; the price carries its date, its figure and its source, and nothing else; a
// name that matches two assets is refused.
#[tokio::test]
async fn agt_071_the_edges_of_a_price_read() {
    use crate::context::asset::DatedClose;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };
    let asset = bench
        .assets
        .get_non_cash_assets()
        .await
        .expect("assets")
        .remove(0);
    bench
        .assets
        .record_daily_closes(
            &asset.id,
            vec![DatedClose {
                date: "2020-01-02".to_string(),
                price: 300 * M,
            }],
        )
        .await
        .expect("fetched");

    let read = call("get_price", json!({ "asset": "CW8" }))
        .await
        .expect("answered");
    assert_eq!(
        read["date"],
        chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    );
    assert_eq!(
        read["price"],
        json!({ "date": "2020-01-02", "price": 300 * M, "source": "YahooFinance" })
    );

    call("add_asset", paris_listing()).await.expect("added");
    assert_eq!(
        call("get_price", json!({ "asset": "CW8" }))
            .await
            .expect_err("refused")
            .code,
        "AssetAmbiguous"
    );
}

// AGT-064 — the edges of an addition: what the form refuses, with its codes and the
// reason of a duplicate; the exchange read with case ignored; a shared reference; a refused
// addition takes no place of the limit; an added asset is no recording.
#[tokio::test]
async fn agt_064_the_edges_of_an_addition() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    for (arguments, code) in [
        (
            json!({ "name": "", "reference": "MC", "currency": "EUR", "kind": "custom" }),
            "MissingArgument",
        ),
        (
            json!({ "name": 7, "reference": "MC", "currency": "EUR", "kind": "custom" }),
            "MissingArgument",
        ),
        (
            json!({ "name": "LVMH", "reference": "", "currency": "EUR", "kind": "custom" }),
            "MissingArgument",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": 7, "kind": "custom" }),
            "MissingArgument",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "class": "cash" }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "kind": 7 }),
            "InvalidArguments",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "kind": "listed", "isin": "FR123" }),
            "InvalidIsinFormat",
        ),
        (
            json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "kind": "listed", "isin": "FR0000121014", "exchange": "NOPE" }),
            "InvalidExchange",
        ),
        (
            json!({ "name": "Flat", "reference": "FLAT", "currency": "EUR", "kind": "custom", "exchange": "XPAR" }),
            "ExchangeNotAllowed",
        ),
        (
            json!({ "name": "Ether", "reference": "ETH", "currency": "EUR", "kind": "crypto", "class": "realestate" }),
            "ClassNotAllowed",
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

    // The exchange is read with case ignored; another asset sharing the reference is said.
    let paris = call(
        "add_asset",
        json!({ "name": "Amundi MSCI World Paris", "reference": "CW8", "currency": "EUR", "kind": "listed", "isin": "LU1681043599", "exchange": "xpar" }),
    )
    .await
    .expect("added");
    assert_eq!(paris["asset"]["exchange"], "XPAR");
    assert_eq!(paris["reference_shared"], true);

    // A duplicate is refused, an archived one included, and the reason names nothing.
    let lvmh = json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "kind": "listed", "isin": "FR0000121014" });
    call("add_asset", lvmh.clone()).await.expect("added");
    let duplicate = call("add_asset", lvmh.clone()).await.expect_err("refused");
    assert_eq!(duplicate.code, "AssetAlreadyExists");
    assert!(!duplicate.message.contains("LVMH") && !duplicate.message.contains("MC"));
    let added = bench
        .assets
        .get_non_cash_assets_by_name(true)
        .await
        .expect("assets")
        .into_iter()
        .find(|asset| asset.name == "LVMH")
        .expect("LVMH");
    bench
        .assets
        .archive_asset(&added.id)
        .await
        .expect("archived");
    assert_eq!(
        call("add_asset", lvmh).await.expect_err("refused").code,
        "AssetAlreadyExists"
    );

    // An added asset is no recording: the session's tools do not name it, and removing
    // everything the session recorded leaves it.
    assert_eq!(
        call(
            "cancel_recording",
            json!({ "transaction": added.id.clone() })
        )
        .await
        .expect_err("refused")
        .code,
        "NotYourRecording"
    );
    bench
        .recorder
        .remove_recorded_in_session(&granted.key)
        .await
        .expect("removed");
    assert!(bench
        .assets
        .get_non_cash_assets_by_name(true)
        .await
        .expect("assets")
        .iter()
        .any(|asset| asset.id == added.id));

    // A refused addition takes no place: with one place left, a refusal leaves it free.
    for _ in 2..super::connections::MAX_RECORDINGS - 1 {
        granted.count_recording();
    }
    assert_eq!(
        call("add_asset", json!({ "name": "LVMH", "reference": "MC", "currency": "EUR", "kind": "listed", "isin": "FR0000121014" }))
            .await
            .expect_err("refused")
            .code,
        "AssetAlreadyExists"
    );
    call(
        "add_asset",
        json!({ "name": "Air Liquide", "reference": "AI", "currency": "EUR", "kind": "custom" }),
    )
    .await
    .expect("the last place");
    assert_eq!(
        call(
            "add_asset",
            json!({ "name": "Bitcoin", "reference": "BTC", "currency": "EUR", "kind": "crypto" }),
        )
        .await
        .expect_err("refused")
        .code,
        "SessionLimitReached"
    );
}

// AGT-063 — the edges of a split and free shares through the tool: the ratio's forms, the
// window's refusals with their codes, no price recorded, and a split removed with the
// session's recordings.
#[tokio::test]
async fn agt_063_the_edges_of_a_split_and_free_shares() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };
    call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2026-02-01" }),
    )
    .await
    .expect("bought");

    for (tool, arguments, code) in [
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": "99999999999999999999", "old": 1 }),
            "InvalidFigure",
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 99_999_999_999_999_i64, "old": 1 }),
            "SplitFactorNotPositive",
        ),
        // The account's cash is never named (CSH-018): a split or free shares of it is not found.
        (
            "record_split",
            json!({ "account": "PEA", "asset": "EUR", "new": 2, "old": 1 }),
            "AssetNotFound",
        ),
        (
            "record_free_shares",
            json!({ "account": "PEA", "asset": "EUR", "quantity": 1 }),
            "AssetNotFound",
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

    // A split that leaves less than a millionth of a share collapses the position.
    call(
        "add_asset",
        json!({ "name": "Tiny", "reference": "TINY", "currency": "EUR", "kind": "custom" }),
    )
    .await
    .expect("added");
    call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "TINY", "quantity": "0.000001", "price": 1, "date": "2026-02-01" }),
    )
    .await
    .expect("bought a millionth");
    assert_eq!(
        call(
            "record_split",
            json!({ "account": "PEA", "asset": "TINY", "new": 1, "old": 2, "date": "2026-03-01" }),
        )
        .await
        .expect_err("refused")
        .code,
        "SplitCollapsesPosition"
    );

    // A sign and spaces around the digits are read; no price is recorded with the split.
    let split = call(
        "record_split",
        json!({ "account": "PEA", "asset": "CW8", "new": " +2 ", "old": "1", "date": "2026-03-01" }),
    )
    .await
    .expect("split");
    assert_eq!(split["transaction"]["quantity"], 2 * M);
    for asset in bench.assets.get_non_cash_assets().await.expect("assets") {
        assert!(bench
            .assets
            .get_asset_prices(&asset.id)
            .await
            .expect("prices")
            .is_empty());
    }

    // All 20 sold, 5 bought again: a split of a half before the sale leaves it oversold,
    // and one while nothing was held splits a closed position.
    for (tool, arguments) in [
        (
            "record_sale",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 20, "price": 60, "date": "2026-05-01" }),
        ),
        (
            "record_purchase",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 5, "price": 60, "date": "2026-07-01" }),
        ),
    ] {
        call(tool, arguments).await.expect("recorded");
    }
    for (arguments, code) in [
        (
            json!({ "account": "PEA", "asset": "CW8", "new": 1, "old": 2, "date": "2026-04-01" }),
            "CascadingOversell",
        ),
        (
            json!({ "account": "PEA", "asset": "CW8", "new": 2, "old": 1, "date": "2026-06-01" }),
            "ClosedPosition",
        ),
    ] {
        assert_eq!(
            call("record_split", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }

    // Removed with the session's recordings, the split with them.
    let removal = bench
        .recorder
        .remove_recorded_in_session(&granted.key)
        .await
        .expect("removed");
    assert_eq!((removal.removed, removal.kept), (5, 0));
    assert!(bench
        .accounts
        .get_transaction_by_id(&recorded_id(&split))
        .await
        .expect("read")
        .is_none());
}

// AGT-063 / AGT-049 — a split that cannot be marked is undone, as any recording.
#[tokio::test]
async fn agt_063_a_split_that_cannot_be_marked_is_undone() {
    use crate::use_cases::holding_transaction::{HoldingTransactionUseCase, Recording, Target};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let (gate, _, pool) = application_with_database(data_dir.path()).await;
    let container = AppContainer::for_headless_writes(pool.clone());
    HoldingTransactionUseCase::new(
        Arc::clone(&container.account_service) as _,
        Arc::clone(&container.asset_service) as _,
    )
    .record_named(
        Recording::Open {
            target: Target {
                account: "PEA".to_string(),
                asset: "CW8".to_string(),
                date: Some("2026-01-05".to_string()),
                quantity: 10 * M,
            },
            total_cost: 4_000 * M,
        },
        "2026-10-14",
    )
    .await
    .expect("the owner's opening balance");
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
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 2, "old": 1, "date": "2026-02-01" }),
        )],
    )
    .await;
    owner.await.expect("owner");

    assert_eq!(answered[0]["result"]["isError"], true);
    let splits: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE transaction_type = 'Split'")
            .fetch_one(&pool)
            .await
            .expect("count");
    assert_eq!(splits, 0, "the split was undone");
}

// AGT-084 — the session counts every transaction it recorded, whatever became of it: a
// cancelling lowers what still exists, never what was recorded.
#[tokio::test]
async fn agt_084_the_session_counts_every_transaction_it_recorded() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let deposit = json!({ "account": "PEA", "amount": 10, "date": "2026-02-01" });
    let first = bench
        .tools
        .call(&granted, "record_deposit", &deposit)
        .await
        .expect("recorded");
    bench
        .tools
        .call(
            &granted,
            "cancel_recording",
            &json!({ "transaction": recorded_id(&first) }),
        )
        .await
        .expect("cancelled");
    bench
        .tools
        .call(&granted, "record_deposit", &deposit)
        .await
        .expect("recorded again");
    // A price and a refused recording are no transaction recorded.
    bench
        .tools
        .call(
            &granted,
            "record_price",
            &json!({ "asset": "CW8", "price": 500, "date": "2026-02-01" }),
        )
        .await
        .expect("priced");
    bench
        .tools
        .call(
            &granted,
            "record_deposit",
            &json!({ "account": "PEA", "amount": 0 }),
        )
        .await
        .expect_err("refused");

    let told = bench.connections.sessions().remove(0);
    assert_eq!((told.transactions_recorded, told.recordings), (2, 1));
}

/// An agent client speaking to the bridge as it goes: what it writes reaches the bridge at
/// once, and what the bridge answers is read line by line.
struct LiveClient {
    writing: tokio::io::DuplexStream,
    reading: tokio::io::BufReader<tokio::io::DuplexStream>,
    served: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl LiveClient {
    fn start(data_dir: &Path) -> Self {
        let (writing, input) = tokio::io::duplex(64 * 1024);
        let (output, reading) = tokio::io::duplex(64 * 1024);
        let connector = reaching(data_dir);
        let served = tokio::spawn(async move { bridge::serve(input, output, connector).await });
        Self {
            writing,
            reading: tokio::io::BufReader::new(reading),
            served,
        }
    }

    async fn say(&mut self, message: Value) {
        use tokio::io::AsyncWriteExt;
        let mut line = message.to_string();
        line.push('\n');
        self.writing.write_all(line.as_bytes()).await.expect("said");
    }

    async fn heard(&mut self) -> Value {
        use tokio::io::AsyncBufReadExt;
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(5), self.reading.read_line(&mut line))
            .await
            .expect("an answer in time")
            .expect("read");
        serde_json::from_str(&line).expect("json")
    }
}

fn cancelled(id: u32) -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/cancelled",
        "params": { "requestId": id, "reason": "the user gave up" } })
}

async fn until(what: &str, mut holds: impl FnMut() -> bool) {
    for _ in 0..400 {
        if holds() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("{what} did not happen");
}

// AGT-085 — a call the agent client cancels while its connection waits for the owner
// withdraws the request: the dialog leaves the window, and the call is not answered.
#[tokio::test]
async fn agt_085_a_cancelled_call_withdraws_its_connection_request() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let mut client = LiveClient::start(data_dir.path());
    client
        .say(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": "claude-code" } } }))
        .await;
    client.heard().await;
    client
        .say(serde_json::from_str(&call(1, "list_accounts", json!({}))).expect("json"))
        .await;
    until("the request", || !gate.connections().requests().is_empty()).await;

    client.say(cancelled(1)).await;
    until("the withdrawal", || {
        gate.connections().requests().is_empty()
    })
    .await;

    // The client goes on: a ping is answered, and the cancelled call never is.
    client
        .say(json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }))
        .await;
    assert_eq!(client.heard().await["id"], 2);
    assert!(gate.connections().sessions().is_empty());
    drop(client.writing);
    client.served.await.expect("served").expect("ended");
}

// AGT-085 — a cancel for a call of a session already allowed changes nothing of it: the
// call is answered, and the session stays.
#[tokio::test]
async fn agt_085_a_cancel_in_an_allowed_session_changes_nothing() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let owner = owner_answers(&gate, true);
    let mut client = LiveClient::start(data_dir.path());
    client
        .say(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": "claude-code" } } }))
        .await;
    client.heard().await;
    client
        .say(serde_json::from_str(&call(1, "list_accounts", json!({}))).expect("json"))
        .await;
    assert_eq!(client.heard().await["id"], 1);
    owner.await.expect("owner");

    client
        .say(serde_json::from_str(&call(2, "list_assets", json!({}))).expect("json"))
        .await;
    client.say(cancelled(2)).await;
    let answered = client.heard().await;
    assert_eq!(answered["id"], 2);
    assert_eq!(answered["result"]["isError"], false);
    assert_eq!(gate.connections().sessions().len(), 1);
}

// AGT-082 / AGT-086 — an agent that goes away while its removal request waits withdraws it.
#[tokio::test]
async fn agt_086_an_agent_that_goes_away_withdraws_its_removal_request() {
    use crate::context::account::JournalFilter;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let (gate, accounts) = application_with_accounts(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let owner = owner_answers(&gate, true);
    let account = accounts.get_all().await.expect("accounts").remove(0);
    let deposit = accounts
        .get_account_journal(&account.id, &JournalFilter::default())
        .await
        .expect("journal")
        .rows
        .remove(0)
        .transaction;
    let mut client = LiveClient::start(data_dir.path());
    client
        .say(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": "claude-code" } } }))
        .await;
    client.heard().await;
    client
        .say(
            serde_json::from_str(&call(
                1,
                "request_removal",
                json!({ "transaction": deposit.id }),
            ))
            .expect("json"),
        )
        .await;
    owner.await.expect("owner");
    until("the removal request", || {
        !gate.connections().removals().is_empty()
    })
    .await;

    drop(client.writing);
    until("the withdrawal", || {
        gate.connections().removals().is_empty()
    })
    .await;
    until("the session's end", || {
        gate.connections().sessions().is_empty()
    })
    .await;
}

// AGT-085 — a line the client writes in part while a call waits holds nothing back: the
// call is answered, and the line is read whole once its end comes.
#[tokio::test]
async fn agt_085_a_line_written_in_part_holds_nothing_back() {
    use tokio::io::AsyncWriteExt;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let mut client = LiveClient::start(data_dir.path());
    client
        .say(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": "claude-code" } } }))
        .await;
    client.heard().await;
    client
        .say(serde_json::from_str(&call(1, "list_accounts", json!({}))).expect("json"))
        .await;
    until("the request", || !gate.connections().requests().is_empty()).await;

    client
        .writing
        .write_all(br#"{"jsonrpc":"2.0","id":5,"me"#)
        .await
        .expect("half a line");
    owner_answers(&gate, true).await.expect("owner");
    assert_eq!(client.heard().await["id"], 1);

    client
        .writing
        .write_all(b"thod\":\"ping\"}\n")
        .await
        .expect("the rest");
    assert_eq!(client.heard().await["id"], 5);
}

/// The owner answers the next change request of `connections`.
async fn change_waiting(connections: &AgentConnections) -> super::connections::ChangeRequest {
    for _ in 0..400 {
        if let Some(request) = connections.changes().into_iter().next() {
            return request;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("no change request arrived");
}

/// The deposit the workbench seeds, as the owner typed it.
async fn typed_deposit(bench: &Workbench) -> crate::context::account::Transaction {
    let account = bench.accounts.get_all().await.expect("accounts").remove(0);
    bench
        .accounts
        .get_account_journal(
            &account.id,
            &crate::context::account::JournalFilter::default(),
        )
        .await
        .expect("journal")
        .rows
        .remove(0)
        .transaction
}

// AGT-087 / AGT-088 / AGT-082 — an agent asks to change a transaction the owner typed: the
// window is shown it as it is and as it would become, a refusal gives the agent nothing and
// changes nothing, the next ask is asked again, and a yes changes it through the window's
// correction.
#[tokio::test]
async fn agt_087_an_agent_asks_and_the_owner_answers_each_change() {
    use super::connections::ShownValue;
    use super::gate::{answer_change, AnswerRefused};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let deposit = typed_deposit(&bench).await;
    let ask = |granted: super::connections::Granted| {
        let bench = Arc::clone(&bench);
        let id = deposit.id.clone();
        tokio::spawn(async move {
            bench
                .tools
                .call(
                    &granted,
                    "request_change",
                    &json!({ "transaction": id, "amount": "4500", "date": "2026-01-03" }),
                )
                .await
        })
    };

    // Asked, and shown as it is and as it would become; the call waits.
    let asked = ask(granted.clone());
    let request = change_waiting(&bench.connections).await;
    assert_eq!(request.client, "claude-code");
    assert_eq!(request.session, granted.session_id);
    assert_eq!(request.transaction.id, deposit.id);
    assert_eq!(request.transaction.date, "2026-01-02");
    assert_eq!(request.transaction.amount, 5_000 * M);
    assert_eq!(request.becomes.id, deposit.id);
    assert_eq!(request.becomes.date, "2026-01-03");
    assert_eq!(request.becomes.amount, 4_500 * M);
    assert_eq!(request.becomes.account, "PEA");
    // A deposit holds its amount as its quantity too.
    assert_eq!(
        request.moved,
        vec![ShownValue::Date, ShownValue::Quantity, ShownValue::Amount]
    );
    assert!(!asked.is_finished());

    // Refused: the agent is given nothing, and nothing is changed.
    answer_change(&bench.connections, &bench.recorder, request.id, false)
        .await
        .expect("answered");
    let refused = asked.await.expect("asked").expect_err("refused");
    assert_eq!(refused.code, "ChangeRefused");
    assert!(bench.connections.changes().is_empty());
    let stored = bench
        .accounts
        .get_transaction_by_id(&deposit.id)
        .await
        .expect("read")
        .expect("kept");
    assert_eq!(stored.total_amount, 5_000 * M);
    assert_eq!(
        answer_change(&bench.connections, &bench.recorder, request.id, true).await,
        Err(AnswerRefused::Gone)
    );

    // Nothing of the refusal is kept: the same change is asked again, and made.
    let asked = ask(granted.clone());
    let request = change_waiting(&bench.connections).await;
    answer_change(&bench.connections, &bench.recorder, request.id, true)
        .await
        .expect("changed");
    let changed = asked.await.expect("asked").expect("changed");
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["transaction"]["id"], json!(deposit.id));
    let stored = bench
        .accounts
        .get_transaction_by_id(&deposit.id)
        .await
        .expect("read")
        .expect("kept");
    assert_eq!(stored.date, "2026-01-03");
    assert_eq!(stored.total_amount, 4_500 * M);

    // A transaction an agent recorded is not asked about.
    let of_the_agent = recorded_id(
        &bench
            .tools
            .call(
                &granted,
                "record_deposit",
                &json!({ "account": "PEA", "amount": 10, "date": "2026-01-04" }),
            )
            .await
            .expect("recorded"),
    );
    let refused = bench
        .tools
        .call(
            &granted,
            "request_change",
            &json!({ "transaction": of_the_agent, "amount": 11 }),
        )
        .await
        .expect_err("refused");
    assert_eq!(refused.code, "NotTypedByOwner");
    assert!(bench.connections.changes().is_empty());
}

// AGT-087 — what is refused before anything is asked: the arguments, the arguments a type
// does not take, and what the window's correction refuses, checked with nothing written.
#[tokio::test]
async fn agt_087_a_change_is_refused_before_asking() {
    use crate::use_cases::holding_transaction::{Recording, Target, Trade, TradeAmount};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let deposit = typed_deposit(&bench).await;
    let target = |date: &str, quantity: i64| Target {
        account: "PEA".to_string(),
        asset: "CW8".to_string(),
        date: Some(date.to_string()),
        quantity,
    };
    let opened = bench
        .recorder
        .record_named(
            Recording::Open {
                target: target("2026-01-05", 10 * M),
                total_cost: 4_000 * M,
            },
            "2026-10-17",
        )
        .await
        .expect("opened");
    bench
        .recorder
        .record_named(
            Recording::Sell(Trade {
                target: target("2026-03-01", 8 * M),
                amount: TradeAmount::Price(500 * M),
                fees: 0,
                rate: M,
                note: None,
            }),
            "2026-10-17",
        )
        .await
        .expect("sold");
    let opening = opened.transaction.id.clone();
    for (arguments, code) in [
        (json!({ "amount": 1 }), "MissingArgument"),
        (json!({ "transaction": deposit.id }), "MissingArgument"),
        (
            json!({ "transaction": "no-such-transaction", "amount": 1 }),
            "TransactionNotFound",
        ),
        (
            json!({ "transaction": deposit.id, "price": 1 }),
            "InvalidArguments",
        ),
        (
            json!({ "transaction": opening, "price": 1, "total": 2 }),
            "InvalidArguments",
        ),
        (
            json!({ "transaction": opening, "fees": 1 }),
            "InvalidArguments",
        ),
        (
            json!({ "transaction": deposit.id, "amount": "1,5" }),
            "InvalidFigure",
        ),
        (
            json!({ "transaction": deposit.id, "date": "yesterday" }),
            "InvalidDate",
        ),
        // What would leave the transaction as it stands changes nothing.
        (
            json!({ "transaction": deposit.id, "amount": 5000, "date": "2026-01-02" }),
            "MissingArgument",
        ),
        // A split's ratio takes both its parts.
        (
            json!({ "transaction": opening, "new": 2 }),
            "InvalidArguments",
        ),
        // The sale of 8 depends on the 10 opened: 5 would oversell it.
        (
            json!({ "transaction": opening, "quantity": 5 }),
            "CascadingOversell",
        ),
    ] {
        let refused = bench
            .tools
            .call(&granted, "request_change", &arguments)
            .await
            .expect_err("refused");
        assert_eq!(refused.code, code, "{arguments}");
    }
    assert!(bench.connections.changes().is_empty(), "nothing was asked");
    let stored = bench
        .accounts
        .get_transaction_by_id(&opening)
        .await
        .expect("read")
        .expect("kept");
    assert_eq!(stored.quantity, 10 * M, "nothing was written");
}

// AGT-087 — a split is changed by its ratio, a purchase's total follows its unit price when
// its quantity changes, as the window's correction makes them.
#[tokio::test]
async fn agt_087_each_type_takes_what_its_correction_takes() {
    use super::gate::answer_change;
    use crate::use_cases::holding_transaction::{Recording, Target, Trade, TradeAmount};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let target = |date: &str, quantity: i64| Target {
        account: "PEA".to_string(),
        asset: "CW8".to_string(),
        date: Some(date.to_string()),
        quantity,
    };
    let bought = bench
        .recorder
        .record_named(
            Recording::Buy(Trade {
                target: target("2026-01-05", 4 * M),
                amount: TradeAmount::Price(100 * M),
                fees: 0,
                rate: M,
                note: None,
            }),
            "2026-10-17",
        )
        .await
        .expect("bought");
    let split = bench
        .recorder
        .record_split(
            &bought.transaction.account_id,
            bought.transaction.asset_id.clone(),
            "2026-02-01".to_string(),
            2 * M,
            None,
        )
        .await
        .expect("split");
    let ask = |arguments: Value| {
        let bench = Arc::clone(&bench);
        let granted = granted.clone();
        tokio::spawn(async move {
            bench
                .tools
                .call(&granted, "request_change", &arguments)
                .await
        })
    };

    let asked = ask(json!({ "transaction": split.id, "new": 3, "old": 1 }));
    let request = change_waiting(&bench.connections).await;
    assert_eq!(request.transaction.quantity, 2 * M);
    assert_eq!(request.becomes.quantity, 3 * M);
    answer_change(&bench.connections, &bench.recorder, request.id, false)
        .await
        .expect("answered");
    asked.await.expect("asked").expect_err("refused");

    let refused = bench
        .tools
        .call(
            &granted,
            "request_change",
            &json!({ "transaction": split.id, "new": 3 }),
        )
        .await
        .expect_err("refused");
    assert_eq!(refused.code, "InvalidArguments");

    let asked = ask(json!({ "transaction": bought.transaction.id, "quantity": 5 }));
    let request = change_waiting(&bench.connections).await;
    assert_eq!(request.transaction.amount, 400 * M);
    assert_eq!(request.becomes.quantity, 5 * M);
    assert_eq!(request.becomes.amount, 500 * M);
    answer_change(&bench.connections, &bench.recorder, request.id, true)
        .await
        .expect("changed");
    asked.await.expect("asked").expect("changed");
}

// AGT-088 — a transaction that changed while its request waited is not changed; a session
// that ends withdraws its request (AGT-082); removal and change requests share one order.
#[tokio::test]
async fn agt_088_a_change_answers_what_was_shown_and_requests_share_one_order() {
    use super::gate::{answer_change, AnswerRefused};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let deposit = typed_deposit(&bench).await;
    let ask = |granted: super::connections::Granted| {
        let bench = Arc::clone(&bench);
        let id = deposit.id.clone();
        tokio::spawn(async move {
            bench
                .tools
                .call(
                    &granted,
                    "request_change",
                    &json!({ "transaction": id, "amount": 4000 }),
                )
                .await
        })
    };

    let asked = ask(granted.clone());
    let request = change_waiting(&bench.connections).await;
    bench
        .recorder
        .correct_transaction(
            &deposit.account_id,
            &deposit.id,
            deposit.date.clone(),
            4_800 * M,
            deposit.unit_price,
            deposit.exchange_rate,
            deposit.fees,
            None,
            None,
        )
        .await
        .expect("corrected in the journal");
    assert_eq!(
        answer_change(&bench.connections, &bench.recorder, request.id, true).await,
        Err(AnswerRefused::NotDone {
            reason: "TransactionChanged".to_string()
        })
    );
    assert_eq!(
        asked.await.expect("asked").expect_err("changed").code,
        "TransactionChanged"
    );
    let stored = bench
        .accounts
        .get_transaction_by_id(&deposit.id)
        .await
        .expect("read")
        .expect("kept");
    assert_eq!(stored.total_amount, 4_800 * M);

    // A note the owner typed meanwhile is not overwritten either: the transaction is
    // checked in full, its note, fees, rate and price included.
    let asked = ask(granted.clone());
    let request = change_waiting(&bench.connections).await;
    bench
        .recorder
        .correct_transaction(
            &deposit.account_id,
            &deposit.id,
            deposit.date.clone(),
            4_800 * M,
            deposit.unit_price,
            deposit.exchange_rate,
            deposit.fees,
            None,
            Some("typed meanwhile".to_string()),
        )
        .await
        .expect("noted in the journal");
    assert_eq!(
        answer_change(&bench.connections, &bench.recorder, request.id, true).await,
        Err(AnswerRefused::NotDone {
            reason: "TransactionChanged".to_string()
        })
    );
    assert_eq!(
        asked.await.expect("asked").expect_err("changed").code,
        "TransactionChanged"
    );

    // One order: a removal asked by another session after this change comes after it.
    let asked = ask(granted.clone());
    let change = change_waiting(&bench.connections).await;
    let other = session(&bench.connections).await;
    let tools = Arc::clone(&bench);
    let id = deposit.id.clone();
    let removal_asked = tokio::spawn(async move {
        tools
            .tools
            .call(&other, "request_removal", &json!({ "transaction": id }))
            .await
    });
    let removal = removal_waiting(&bench.connections).await;
    assert!(change.id < removal.id);

    // The session ends: its change request is withdrawn and its call refused.
    assert!(bench.connections.end(granted.session_id));
    assert_eq!(
        asked.await.expect("asked").expect_err("withdrawn").code,
        "ChangeRefused"
    );
    assert!(bench.connections.changes().is_empty());
    bench.connections.end_all();
    assert_eq!(
        removal_asked
            .await
            .expect("asked")
            .expect_err("withdrawn")
            .code,
        "RemovalRefused"
    );
}

// AGT-070 / AGT-071 / AGT-080 / AGT-081 — a portfolio that cannot be read is refused with
// `DatabaseError`: a listing, a price, a removal asked, and the owner's yes to one waiting.
#[tokio::test]
async fn agt_080_a_portfolio_that_cannot_be_read_is_a_database_error() {
    use super::gate::{answer_removal, AnswerRefused};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let deposit = typed_deposit(&bench).await;

    let tools = Arc::clone(&bench);
    let waiting_granted = granted.clone();
    let id = deposit.id.clone();
    let asked = tokio::spawn(async move {
        tools
            .tools
            .call(
                &waiting_granted,
                "request_removal",
                &json!({ "transaction": id }),
            )
            .await
    });
    let request = removal_waiting(&bench.connections).await;

    for table in ["transactions", "asset_prices"] {
        sqlx::query(&format!("ALTER TABLE {table} RENAME TO {table}_unreadable"))
            .execute(&bench.pool)
            .await
            .expect("made unreadable");
    }

    assert_eq!(
        answer_removal(&bench.connections, &bench.recorder, request.id, true).await,
        Err(AnswerRefused::Unreadable)
    );
    assert_eq!(
        asked.await.expect("asked").expect_err("unreadable").code,
        "DatabaseError"
    );
    for (tool, arguments) in [
        ("list_transactions", json!({ "account": "PEA" })),
        ("get_price", json!({ "asset": "CW8", "date": "2026-01-02" })),
        ("request_removal", json!({ "transaction": deposit.id })),
    ] {
        let refused = bench
            .tools
            .call(&granted, tool, &arguments)
            .await
            .expect_err("unreadable");
        assert_eq!(refused.code, "DatabaseError", "{tool}");
    }
}

// AGT-080 / AGT-081 — a removal request takes no place of the session's limit: a session at
// its limit still asks; two sessions may ask for one transaction, and once it is removed the
// other's yes finds it no more; a split's quantity is shown as its factor.
#[tokio::test]
async fn agt_080_two_sessions_ask_for_one_transaction_outside_the_limit() {
    use super::connections::MAX_RECORDINGS;
    use super::gate::{answer_removal, AnswerRefused};
    use crate::use_cases::holding_transaction::{Recording, Target};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let first = session(&bench.connections).await;
    let second = session(&bench.connections).await;
    for _ in 0..MAX_RECORDINGS {
        first.count_recording();
    }
    let opened = bench
        .recorder
        .record_named(
            Recording::Open {
                target: Target {
                    account: "PEA".to_string(),
                    asset: "CW8".to_string(),
                    date: Some("2026-01-05".to_string()),
                    quantity: 10 * M,
                },
                total_cost: 4_000 * M,
            },
            "2026-10-17",
        )
        .await
        .expect("opened");
    let split = bench
        .recorder
        .record_split(
            &opened.transaction.account_id,
            opened.transaction.asset_id.clone(),
            "2026-02-01".to_string(),
            3 * M,
            None,
        )
        .await
        .expect("split");
    let ask = |granted: super::connections::Granted| {
        let bench = Arc::clone(&bench);
        let id = split.id.clone();
        tokio::spawn(async move {
            bench
                .tools
                .call(&granted, "request_removal", &json!({ "transaction": id }))
                .await
        })
    };

    let by_first = ask(first.clone());
    removal_waiting(&bench.connections).await;
    let by_second = ask(second.clone());
    until("two removal requests", || {
        bench.connections.removals().len() == 2
    })
    .await;
    let requests = bench.connections.removals();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].session, first.session_id);
    assert_eq!(requests[0].transaction.quantity, 3 * M, "a split's factor");

    answer_removal(&bench.connections, &bench.recorder, requests[0].id, true)
        .await
        .expect("removed");
    by_first.await.expect("asked").expect("removed");
    assert_eq!(
        answer_removal(&bench.connections, &bench.recorder, requests[1].id, true).await,
        Err(AnswerRefused::NotDone {
            reason: "TransactionNotFound".to_string()
        })
    );
    assert_eq!(
        by_second.await.expect("asked").expect_err("gone").code,
        "TransactionNotFound"
    );
}

// AGT-052 — a recording the owner deletes in the journal is not told at once: the session's
// next write tells what is left.
#[tokio::test]
async fn agt_052_a_deletion_in_the_journal_is_told_at_the_next_write() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };

    let first = recorded_id(
        &call(
            "record_deposit",
            json!({ "account": "PEA", "amount": 10, "date": "2026-02-01" }),
        )
        .await
        .expect("recorded"),
    );
    let deleted = bench
        .accounts
        .get_transaction_by_id(&first)
        .await
        .expect("read")
        .expect("recorded");
    bench
        .recorder
        .cancel_transaction(&deleted.account_id, &deleted.id)
        .await
        .expect("deleted in the journal");
    assert_eq!(
        bench.connections.sessions().remove(0).recordings,
        1,
        "not told yet"
    );

    call(
        "record_deposit",
        json!({ "account": "PEA", "amount": 20, "date": "2026-02-02" }),
    )
    .await
    .expect("recorded");
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 1);
    assert_eq!(
        told.last_recording.expect("a last recording").date,
        "2026-02-02"
    );
}

/// A live client that said hello and whose first call waits for the owner's answer.
async fn waiting_for_the_owner(data_dir: &Path, gate: &Arc<AgentGate>) -> LiveClient {
    let mut client = LiveClient::start(data_dir);
    client
        .say(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": "claude-code" } } }))
        .await;
    client.heard().await;
    client
        .say(serde_json::from_str(&call(1, "list_accounts", json!({}))).expect("json"))
        .await;
    until("the connection request", || {
        !gate.connections().requests().is_empty()
    })
    .await;
    client
}

// AGT-085 — a cancel naming no call being answered is ignored: the connection request
// still waits, and the call is answered once the owner allows.
#[tokio::test]
async fn agt_085_a_cancel_naming_no_call_is_ignored() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let mut client = waiting_for_the_owner(data_dir.path(), &gate).await;

    client.say(cancelled(99)).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(gate.connections().requests().len(), 1, "still waiting");

    owner_answers(&gate, true).await.expect("owner");
    let answered = client.heard().await;
    assert_eq!(answered["id"], 1);
    assert_eq!(answered["result"]["isError"], false);
}

// AGT-085 — past 64 lines sent while a call waits, the bridge stops reading the agent: a
// cancel sent after them is not read, so the connection request still waits; it is read
// once the call is answered.
#[tokio::test]
async fn agt_085_the_bridge_stops_reading_after_64_waiting_lines() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let mut client = waiting_for_the_owner(data_dir.path(), &gate).await;

    for _ in 0..64 {
        client
            .say(json!({ "jsonrpc": "2.0", "method": "notifications/progress" }))
            .await;
    }
    client.say(cancelled(1)).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        gate.connections().requests().len(),
        1,
        "the cancel past the 64 lines was not read"
    );

    owner_answers(&gate, true).await.expect("owner");
    assert_eq!(client.heard().await["id"], 1);
}

// AGT-085 — up to 64 lines are read while a call waits: a cancel that is the 64th is read,
// and withdraws the connection request.
#[tokio::test]
async fn agt_085_the_64th_waiting_line_is_still_read() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let gate = application(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let mut client = waiting_for_the_owner(data_dir.path(), &gate).await;

    for _ in 0..63 {
        client
            .say(json!({ "jsonrpc": "2.0", "method": "notifications/progress" }))
            .await;
    }
    client.say(cancelled(1)).await;
    until("the withdrawal", || {
        gate.connections().requests().is_empty()
    })
    .await;
}

// AGT-085 — once granted, a cancel of a call whose removal request waits on the owner
// changes nothing: the request still waits, and the call is answered with the owner's
// refusal.
#[tokio::test]
async fn agt_085_a_cancel_while_a_removal_request_waits_changes_nothing() {
    use super::connections::RemovalAnswer;
    use crate::context::account::JournalFilter;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let (gate, accounts) = application_with_accounts(data_dir.path()).await;
    gate.allow(true).expect("allowed");
    let owner = owner_answers(&gate, true);
    let account = accounts.get_all().await.expect("accounts").remove(0);
    let deposit = accounts
        .get_account_journal(&account.id, &JournalFilter::default())
        .await
        .expect("journal")
        .rows
        .remove(0)
        .transaction;
    let mut client = LiveClient::start(data_dir.path());
    client
        .say(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": "claude-code" } } }))
        .await;
    client.heard().await;
    client
        .say(
            serde_json::from_str(&call(
                1,
                "request_removal",
                json!({ "transaction": deposit.id }),
            ))
            .expect("json"),
        )
        .await;
    owner.await.expect("owner");
    until("the removal request", || {
        !gate.connections().removals().is_empty()
    })
    .await;

    client.say(cancelled(1)).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let waiting = gate.connections().removals();
    assert_eq!(waiting.len(), 1, "the cancel changed nothing");

    gate.connections()
        .take_removal(waiting[0].id)
        .expect("waiting")
        .tell(RemovalAnswer::Refused);
    let answered = client.heard().await;
    assert_eq!(answered["id"], 1);
    assert_eq!(answered["result"]["isError"], true);
    assert!(answered["result"]["content"][0]["text"]
        .as_str()
        .is_some_and(|text| text.contains("did not remove")));
    assert_eq!(gate.connections().sessions().len(), 1);
}

/// A live client in a session the owner allowed, its first call answered.
async fn allowed_client(data_dir: &Path, gate: &Arc<AgentGate>) -> LiveClient {
    let owner = owner_answers(gate, true);
    let mut client = LiveClient::start(data_dir);
    client
        .say(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": { "clientInfo": { "name": "claude-code" } } }))
        .await;
    client.heard().await;
    client
        .say(serde_json::from_str(&call(1, "list_accounts", json!({}))).expect("json"))
        .await;
    assert_eq!(client.heard().await["id"], 1);
    owner.await.expect("owner");
    client
}

/// The window's events until `wanted` of them are `TransactionUpdated`.
async fn transactions_told(window: &mut EventReceiver, wanted: usize) -> Vec<Event> {
    let mut told = Vec::new();
    for _ in 0..400 {
        told.extend(window.drain());
        if told
            .iter()
            .filter(|event| **event == Event::TransactionUpdated)
            .count()
            >= wanted
        {
            return told;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("the window was told {told:?}");
}

// AGT-036 / AGT-081 — the owner's removal of a transaction an agent asked about publishes
// `TransactionUpdated`; a recording that cannot be marked publishes twice — once written,
// once undone — though the call is refused (AGT-049).
#[tokio::test]
async fn agt_036_the_owners_removal_and_an_undone_recording_reach_the_window() {
    use super::gate::answer_removal;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let mut on_bus = owner_on_bus(data_dir.path()).await;
    on_bus.gate.allow(true).expect("allowed");
    let typed: String = sqlx::query_scalar("SELECT id FROM transactions LIMIT 1")
        .fetch_one(&on_bus.pool)
        .await
        .expect("the seeded deposit");

    // The owner's removal on an agent's request.
    let mut client = allowed_client(data_dir.path(), &on_bus.gate).await;
    on_bus.window.drain();
    client
        .say(
            serde_json::from_str(&call(2, "request_removal", json!({ "transaction": typed })))
                .expect("json"),
        )
        .await;
    until("the removal request", || {
        !on_bus.gate.connections().removals().is_empty()
    })
    .await;
    let request = on_bus.gate.connections().removals().remove(0);
    assert!(
        !on_bus.window.drain().contains(&Event::TransactionUpdated),
        "nothing is told before the owner's answer"
    );
    answer_removal(
        on_bus.gate.connections(),
        &on_bus.recorder,
        request.id,
        true,
    )
    .await
    .expect("removed");
    assert_eq!(client.heard().await["id"], 2);
    transactions_told(&mut on_bus.window, 1).await;

    drop(client.writing);
    until("the session's end", || {
        on_bus.gate.connections().sessions().is_empty()
    })
    .await;

    // AGT-049 — a recording that cannot be marked is written, then undone: two events.
    sqlx::query("DROP TABLE agent_recordings")
        .execute(&on_bus.pool)
        .await
        .expect("dropped");
    let mut client = allowed_client(data_dir.path(), &on_bus.gate).await;
    on_bus.window.drain();
    client
        .say(
            serde_json::from_str(&call(
                2,
                "record_deposit",
                json!({ "account": "PEA", "amount": 10, "date": "2026-02-02" }),
            ))
            .expect("json"),
        )
        .await;
    let refused = client.heard().await;
    assert_eq!(refused["result"]["isError"], true);
    let mut told = transactions_told(&mut on_bus.window, 2).await;
    // Settled: no third event follows.
    tokio::time::sleep(Duration::from_millis(100)).await;
    told.extend(on_bus.window.drain());
    assert_eq!(
        told.iter()
            .filter(|event| **event == Event::TransactionUpdated)
            .count(),
        2,
        "{told:?}"
    );
}

/// What a write would change: the rows of every table a recording, a holding, an asset
/// seeded or a change recorded for sync touches.
async fn written_rows(bench: &Workbench) -> Vec<i64> {
    let mut sizes = Vec::new();
    for table in [
        "transactions",
        "holdings",
        "assets",
        "changes",
        "agent_recordings",
    ] {
        let rows: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&bench.pool)
            .await
            .expect("counted");
        sizes.push(rows);
    }
    sizes
}

// AGT-072 — a preview of each of the four recordings writes nothing and answers the
// transaction as it would be recorded, and the account on the evening of its date without
// it and with it.
#[tokio::test]
async fn agt_072_a_preview_changes_nothing_and_shows_the_account_before_and_after() {
    use crate::use_cases::holding_transaction::{Recording, Target, Trade, TradeAmount};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let preview = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move {
            tools
                .call(
                    granted,
                    "preview_recording",
                    &json!({ "tool": tool, "arguments": arguments }),
                )
                .await
        }
    };
    let written = written_rows(&bench).await;

    let bought = preview(
        "record_purchase",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 2, "price": 100, "date": "2026-02-01" }),
    )
    .await
    .expect("previewed");
    assert_eq!(bought["status"], "previewed");
    assert_eq!(bought["transaction"]["transaction_type"], "Purchase");
    assert_eq!(bought["transaction"]["total_amount"], 200 * M);
    assert_eq!(bought["transaction"]["id"], Value::Null);
    assert_eq!(bought["account"], "PEA");
    assert_eq!(bought["currency"], "EUR");
    assert_eq!(bought["date"], "2026-02-01");
    assert_eq!(bought["before"]["cash"], 5_000 * M);
    assert_eq!(bought["before"]["holdings"], json!([]));
    assert_eq!(bought["after"]["cash"], 4_800 * M);
    assert_eq!(
        bought["after"]["holdings"],
        json!([{ "asset": "CW8", "name": "Amundi MSCI World", "quantity": 2 * M, "average_price": 100 * M }])
    );

    let opened = preview(
        "record_opening_balance",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "total_cost": 4000, "date": "2026-01-05" }),
    )
    .await
    .expect("previewed");
    assert_eq!(opened["transaction"]["transaction_type"], "OpeningBalance");
    assert_eq!(
        opened["after"]["holdings"],
        json!([{ "asset": "CW8", "name": "Amundi MSCI World", "quantity": 10 * M, "average_price": 400 * M }])
    );
    assert_eq!(written_rows(&bench).await, written, "nothing was written");

    // A position to sell and split.
    bench
        .recorder
        .record_named(
            Recording::Buy(Trade {
                target: Target {
                    account: "PEA".to_string(),
                    asset: "CW8".to_string(),
                    date: Some("2026-02-01".to_string()),
                    quantity: 4 * M,
                },
                amount: TradeAmount::Price(100 * M),
                fees: 0,
                rate: M,
                note: None,
            }),
            "2026-10-17",
        )
        .await
        .expect("bought");
    let written = written_rows(&bench).await;
    let sold = preview(
        "record_sale",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "price": 150, "date": "2026-03-01" }),
    )
    .await
    .expect("previewed");
    assert_eq!(sold["before"]["holdings"][0]["quantity"], 4 * M);
    assert_eq!(sold["after"]["holdings"][0]["quantity"], 3 * M);
    assert_eq!(sold["after"]["cash"], 4_750 * M);
    let split = preview(
        "record_split",
        json!({ "account": "PEA", "asset": "CW8", "new": 2, "old": 1, "date": "2026-03-01" }),
    )
    .await
    .expect("previewed");
    assert_eq!(split["transaction"]["transaction_type"], "Split");
    assert_eq!(split["after"]["holdings"][0]["quantity"], 8 * M);
    assert_eq!(written_rows(&bench).await, written, "nothing was written");
}

// AGT-072 / DEBT-135 — an account built from opening balances alone holds no transaction on
// its cash line: the cash its sale brings is still shown, as the account will hold it.
#[tokio::test]
async fn agt_072_the_cash_of_an_account_built_from_opening_balances_is_previewed() {
    use crate::use_cases::holding_transaction::{Recording, Target};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let accounts: Arc<dyn AccountServiceContract> = Arc::clone(&bench.accounts) as _;
    let assets: Arc<dyn AssetServiceContract> = Arc::clone(&bench.assets) as _;
    AccountCreationUseCase::new(accounts, assets, Arc::clone(&bench.transactions))
        .create("CTO".to_string(), String::new(), "EUR".to_string(), false)
        .await
        .expect("account");
    bench
        .recorder
        .record_named(
            Recording::Open {
                target: Target {
                    account: "CTO".to_string(),
                    asset: "CW8".to_string(),
                    date: Some("2026-01-05".to_string()),
                    quantity: 10 * M,
                },
                total_cost: 100 * M,
            },
            "2026-10-17",
        )
        .await
        .expect("opened");
    let granted = session(&bench.connections).await;

    let sold = bench
        .tools
        .call(
            &granted,
            "preview_recording",
            &json!({ "tool": "record_sale", "arguments": { "account": "CTO", "asset": "CW8", "quantity": 10, "price": 15, "date": "2026-03-01" } }),
        )
        .await
        .expect("previewed");

    assert_eq!(sold["before"]["cash"], 0);
    assert_eq!(sold["after"]["cash"], 150 * M);
    assert_eq!(sold["after"]["holdings"], json!([]));
}

// AGT-072 — a preview gives the refusal the real call gives, for each kind; a tool it does
// not preview is refused, naming the four.
#[tokio::test]
async fn agt_072_a_preview_gives_the_refusal_of_the_real_call() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    for (tool, arguments) in [
        (
            "record_sale",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1000, "price": 1, "date": "2026-03-01" }),
        ),
        (
            "record_purchase",
            json!({ "account": "PEA", "asset": "NOPE", "quantity": 1, "price": 1, "date": "2026-03-01" }),
        ),
        (
            "record_purchase",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "price": 1000000, "date": "2026-03-01" }),
        ),
        (
            "record_opening_balance",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 0, "total_cost": 10, "date": "2026-01-05" }),
        ),
        (
            "record_split",
            json!({ "account": "PEA", "asset": "CW8", "new": 2, "old": 1, "date": "2026-03-01" }),
        ),
        (
            "record_purchase",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "price": 1, "date": "2099-01-01" }),
        ),
        (
            "record_transfer",
            json!({ "from": "PEA", "to": "PEA", "asset": "CW8", "quantity": 1, "date": "2026-03-01" }),
        ),
        (
            "record_transfer",
            json!({ "from": "PEA", "to": "NOPE", "asset": "CW8", "quantity": 1, "date": "2026-03-01" }),
        ),
    ] {
        let previewed = bench
            .tools
            .call(
                &granted,
                "preview_recording",
                &json!({ "tool": tool, "arguments": arguments }),
            )
            .await
            .expect_err("refused");
        let real = bench
            .tools
            .call(&granted, tool, &arguments)
            .await
            .expect_err("refused");
        assert_eq!(previewed.code, real.code, "{tool} {arguments}");
        assert_eq!(previewed.message, real.message, "{tool} {arguments}");
    }

    for (arguments, code) in [
        (json!({ "arguments": {} }), "MissingArgument"),
        (
            json!({ "tool": "add_asset", "arguments": {} }),
            "InvalidArguments",
        ),
        (
            json!({ "tool": "record_purchase", "arguments": "CW8" }),
            "InvalidArguments",
        ),
    ] {
        let refused = bench
            .tools
            .call(&granted, "preview_recording", &arguments)
            .await
            .expect_err("refused");
        assert_eq!(refused.code, code, "{arguments}");
    }
    let refused = bench
        .tools
        .call(
            &granted,
            "preview_recording",
            &json!({ "tool": "add_asset", "arguments": {} }),
        )
        .await
        .expect_err("refused");
    for named in [
        "record_purchase",
        "record_sale",
        "record_opening_balance",
        "record_split",
    ] {
        assert!(refused.message.contains(named), "{}", refused.message);
    }
}

// AGT-072 — a preview counts as one of the session's reads, takes no place of its limit and
// is not announced: a session at its limit still previews.
#[tokio::test]
async fn agt_072_a_preview_is_a_read_outside_the_limit() {
    use super::connections::MAX_RECORDINGS;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    for _ in 0..MAX_RECORDINGS {
        granted.count_recording();
    }
    let before = bench.connections.sessions().remove(0);

    bench
        .tools
        .call(
            &granted,
            "preview_recording",
            &json!({ "tool": "record_purchase", "arguments": { "account": "PEA", "asset": "CW8", "quantity": 1, "price": 1, "date": "2026-03-01" } }),
        )
        .await
        .expect("previewed at the limit");
    let after = bench.connections.sessions().remove(0);
    assert_eq!(after.reads, before.reads + 1);
    assert_eq!(after.transactions_recorded, before.transactions_recorded);
    assert_eq!(after.recordings, before.recordings);
}

// AGT-072 / TODO-090 — a deposit, a withdrawal, a dividend and free shares are previewed as
// the four recordings are: nothing written, the account on the evening of the date without
// them and with them.
#[tokio::test]
async fn agt_072_cash_a_dividend_and_free_shares_are_previewed() {
    use crate::use_cases::holding_transaction::{Recording, Target, Trade, TradeAmount};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    bench
        .recorder
        .record_named(
            Recording::Buy(Trade {
                target: Target {
                    account: "PEA".to_string(),
                    asset: "CW8".to_string(),
                    date: Some("2026-02-01".to_string()),
                    quantity: 4 * M,
                },
                amount: TradeAmount::Price(100 * M),
                fees: 0,
                rate: M,
                note: None,
            }),
            "2026-10-17",
        )
        .await
        .expect("bought");
    let written = written_rows(&bench).await;
    let preview = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move {
            tools
                .call(
                    granted,
                    "preview_recording",
                    &json!({ "tool": tool, "arguments": arguments }),
                )
                .await
                .expect("previewed")
        }
    };

    let deposit = preview(
        "record_deposit",
        json!({ "account": "PEA", "amount": 100, "date": "2026-03-01" }),
    )
    .await;
    assert_eq!(deposit["transaction"]["transaction_type"], "Deposit");
    assert_eq!(deposit["before"]["cash"], 4_600 * M);
    assert_eq!(deposit["after"]["cash"], 4_700 * M);

    let withdrawal = preview(
        "record_withdrawal",
        json!({ "account": "PEA", "amount": 600, "date": "2026-03-01" }),
    )
    .await;
    assert_eq!(withdrawal["after"]["cash"], 4_000 * M);

    let dividend = preview(
        "record_dividend",
        json!({ "account": "PEA", "asset": "CW8", "amount": 12, "date": "2026-03-01" }),
    )
    .await;
    assert_eq!(dividend["transaction"]["transaction_type"], "Dividend");
    assert_eq!(dividend["after"]["cash"], 4_612 * M);
    assert_eq!(dividend["after"]["holdings"][0]["quantity"], 4 * M);

    let free_shares = preview(
        "record_free_shares",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "date": "2026-03-01" }),
    )
    .await;
    assert_eq!(free_shares["transaction"]["transaction_type"], "FreeShares");
    assert_eq!(free_shares["after"]["holdings"][0]["quantity"], 5 * M);
    assert_eq!(written_rows(&bench).await, written, "nothing was written");
}

// AGT-072 / TODO-090 — a price is previewed with the price it would replace at that date,
// or none; nothing is written.
#[tokio::test]
async fn agt_072_a_price_is_previewed_with_the_price_it_would_replace() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let preview = |date: &'static str| {
        let (tools, granted) = (&bench.tools, &granted);
        async move {
            tools
                .call(
                    granted,
                    "preview_recording",
                    &json!({ "tool": "record_price", "arguments": { "asset": "CW8", "price": "512.5", "date": date } }),
                )
                .await
                .expect("previewed")
        }
    };

    let first = preview("2026-03-02").await;
    assert_eq!(first["status"], "previewed");
    assert_eq!(first["price"]["price"], 512_500_000);
    assert_eq!(first["asset"], "CW8");
    assert_eq!(first["replaces"], Value::Null);

    bench
        .tools
        .call(
            &granted,
            "record_price",
            &json!({ "asset": "CW8", "price": "500", "date": "2026-03-02" }),
        )
        .await
        .expect("recorded");
    let prices = bench
        .assets
        .get_asset_prices(
            &bench
                .assets
                .get_all_assets()
                .await
                .expect("assets")
                .into_iter()
                .find(|asset| asset.reference == "CW8")
                .expect("CW8")
                .id,
        )
        .await
        .expect("prices")
        .len();
    let second = preview("2026-03-02").await;
    assert_eq!(second["replaces"]["price"], 500 * M);
    let after = bench
        .assets
        .get_asset_prices(
            &bench
                .assets
                .get_all_assets()
                .await
                .expect("assets")
                .into_iter()
                .find(|asset| asset.reference == "CW8")
                .expect("CW8")
                .id,
        )
        .await
        .expect("prices");
    assert_eq!(after.len(), prices, "nothing was written");
    assert_eq!(after[0].price, 500 * M);
}

// AGT-072 / TODO-090 — a correction of what the session recorded is previewed: the account
// on the evening of the corrected date without the correction and with it; nothing written.
#[tokio::test]
async fn agt_072_a_correction_is_previewed() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let bought = recorded_id(
        &bench
            .tools
            .call(
                &granted,
                "record_purchase",
                &json!({ "account": "PEA", "asset": "CW8", "quantity": 2, "price": 100, "date": "2026-02-01" }),
            )
            .await
            .expect("recorded"),
    );
    let written = written_rows(&bench).await;

    let corrected = bench
        .tools
        .call(
            &granted,
            "preview_recording",
            &json!({ "tool": "correct_recording", "arguments": { "transaction": bought, "quantity": 3 } }),
        )
        .await
        .expect("previewed");
    assert_eq!(corrected["transaction"]["id"], json!(bought));
    assert_eq!(corrected["transaction"]["quantity"], 3 * M);
    assert_eq!(corrected["before"]["holdings"][0]["quantity"], 2 * M);
    assert_eq!(corrected["after"]["holdings"][0]["quantity"], 3 * M);
    assert_eq!(corrected["after"]["cash"], 4_700 * M);
    assert_eq!(written_rows(&bench).await, written, "nothing was written");
}

// AGT-072 / TODO-090 — each kind gives the refusal the real call gives.
#[tokio::test]
async fn agt_072_the_rest_gives_the_refusal_of_the_real_call() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let deposit = recorded_id(
        &bench
            .tools
            .call(
                &granted,
                "record_deposit",
                &json!({ "account": "PEA", "amount": 10, "date": "2026-02-01" }),
            )
            .await
            .expect("recorded"),
    );
    for (tool, arguments) in [
        (
            "record_withdrawal",
            json!({ "account": "PEA", "amount": 999999, "date": "2026-03-01" }),
        ),
        ("record_deposit", json!({ "account": "NOPE", "amount": 1 })),
        ("record_deposit", json!({ "account": "PEA", "amount": 0 })),
        (
            "record_dividend",
            json!({ "account": "PEA", "asset": "CW8", "amount": 1, "date": "2026-03-01" }),
        ),
        (
            "record_free_shares",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 1, "date": "2026-03-01" }),
        ),
        ("record_price", json!({ "asset": "NOPE", "price": 1 })),
        (
            "record_price",
            json!({ "asset": "CW8", "price": 0, "date": "2026-03-01" }),
        ),
        (
            "record_price",
            json!({ "asset": "CW8", "price": 1, "date": "2099-01-01" }),
        ),
        (
            "correct_recording",
            json!({ "transaction": deposit, "amount": 1, "quantity": 2 }),
        ),
        (
            "correct_recording",
            json!({ "transaction": "no-such-transaction", "quantity": 2 }),
        ),
    ] {
        let previewed = bench
            .tools
            .call(
                &granted,
                "preview_recording",
                &json!({ "tool": tool, "arguments": arguments }),
            )
            .await
            .expect_err("refused");
        let real = bench
            .tools
            .call(&granted, tool, &arguments)
            .await
            .expect_err("refused");
        assert_eq!(previewed.code, real.code, "{tool} {arguments}");
        assert_eq!(previewed.message, real.message, "{tool} {arguments}");
    }
}

// AGT-065A — a session records a transfer through the window's transfer (TRF): refused as
// the window refuses it, previewed on both accounts, each part marked and counted, never
// corrected part by part, cancelled whole, and removed with the session's recordings.
#[tokio::test]
async fn agt_065a_a_session_records_a_transfer() {
    use crate::context::account::TransactionType;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let call = |tool: &'static str, arguments: Value| {
        let (tools, granted) = (&bench.tools, &granted);
        async move { tools.call(granted, tool, &arguments).await }
    };
    let accounts: Arc<dyn AccountServiceContract> = Arc::clone(&bench.accounts) as _;
    let assets: Arc<dyn AssetServiceContract> = Arc::clone(&bench.assets) as _;
    AccountCreationUseCase::new(accounts, assets, Arc::clone(&bench.transactions))
        .create(
            "Wallet".to_string(),
            String::new(),
            "EUR".to_string(),
            false,
        )
        .await
        .expect("account");
    call(
        "record_deposit",
        json!({ "account": "PEA", "amount": 2000, "date": "2019-01-15" }),
    )
    .await
    .expect("deposited");
    call(
        "record_purchase",
        json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2019-03-01" }),
    )
    .await
    .expect("bought");

    let base = json!({ "from": "PEA", "to": "Wallet", "asset": "CW8", "quantity": 4, "date": "2019-06-01" });
    let with = |extra: Value| {
        let mut arguments = base.clone();
        for (key, value) in extra.as_object().expect("an object") {
            arguments[key] = value.clone();
        }
        arguments
    };
    for (arguments, code) in [
        (
            json!({ "to": "Wallet", "asset": "CW8", "quantity": 1 }),
            "MissingArgument",
        ),
        (with(json!({ "to": "PEA" })), "TransferToSameAccount"),
        (with(json!({ "to": "NOPE" })), "AccountNotFound"),
        (with(json!({ "asset": "NOPE" })), "AssetNotFound"),
        (with(json!({ "quantity": 0 })), "QuantityNotPositive"),
        (with(json!({ "received": 5 })), "TransferReceivedAboveSent"),
        (with(json!({ "rate": 2 })), "TransferRateNotOne"),
        (with(json!({ "date": "2019-02-01" })), "ClosedPosition"),
        (with(json!({ "quantity": 20 })), "Oversell"),
    ] {
        assert_eq!(
            call("record_transfer", arguments.clone())
                .await
                .expect_err("refused")
                .code,
            code,
            "{arguments}"
        );
    }

    // Previewed on both accounts, nothing written.
    let previewed = call(
        "preview_recording",
        json!({ "tool": "record_transfer", "arguments": with(json!({ "received": "3.5" })) }),
    )
    .await
    .expect("previewed");
    assert_eq!(previewed["status"], "previewed");
    let kinds: Vec<&str> = previewed["transactions"]
        .as_array()
        .expect("parts")
        .iter()
        .map(|part| {
            assert!(part["id"].is_null());
            part["transaction_type"].as_str().expect("a type")
        })
        .collect();
    assert_eq!(kinds, ["WriteOff", "TransferOut", "TransferIn"]);
    assert_eq!(previewed["from"]["account"], "PEA");
    assert_eq!(previewed["from"]["after"]["holdings"][0]["quantity"], 6 * M);
    assert_eq!(
        previewed["from"]["after"]["cash"],
        previewed["from"]["before"]["cash"]
    );
    assert_eq!(previewed["to"]["account"], "Wallet");
    assert_eq!(previewed["to"]["before"]["holdings"], json!([]));
    assert_eq!(
        previewed["to"]["after"]["holdings"][0]["quantity"],
        3_500_000_i64
    );
    assert_eq!(
        previewed["to"]["after"]["holdings"][0]["average_price"],
        100 * M
    );
    assert_eq!(
        bench.connections.sessions()[0].recordings,
        2,
        "nothing written"
    );

    let recorded = call(
        "record_transfer",
        with(json!({ "received": "3.5", "note": "to cold storage" })),
    )
    .await
    .expect("recorded");
    assert_eq!(recorded["status"], "recorded");
    assert_eq!(
        [
            &recorded["from"],
            &recorded["from_currency"],
            &recorded["to"],
            &recorded["to_currency"],
            &recorded["asset"]
        ],
        [
            &json!("PEA"),
            &json!("EUR"),
            &json!("Wallet"),
            &json!("EUR"),
            &json!("CW8")
        ]
    );
    let parts = recorded["transactions"].as_array().expect("parts").clone();
    assert_eq!(parts.len(), 3);
    for part in &parts {
        let mark = bench
            .accounts
            .agent_recording_of(part["id"].as_str().expect("an id"))
            .await
            .expect("read")
            .expect("a mark");
        assert_eq!(mark.session, granted.key);
        assert_eq!(part["note"], "to cold storage");
    }
    let told = bench.connections.sessions().remove(0);
    assert_eq!(told.recordings, 5, "each part is a transaction recorded");
    assert_eq!(told.transactions_recorded, 5);
    assert_eq!(
        told.last_recording.expect("a last recording").kind,
        TransactionType::TransferIn
    );

    // A part is not corrected alone (TRF-031); cancelling any part cancels the transfer.
    let part_id = |kind: &str| {
        parts
            .iter()
            .find(|part| part["transaction_type"] == kind)
            .and_then(|part| part["id"].as_str())
            .expect("the part")
            .to_string()
    };
    assert_eq!(
        call(
            "correct_recording",
            json!({ "transaction": part_id("TransferIn"), "quantity": 1 }),
        )
        .await
        .expect_err("refused")
        .code,
        "PartOfTransfer"
    );
    call(
        "cancel_recording",
        json!({ "transaction": part_id("TransferOut") }),
    )
    .await
    .expect("cancelled");
    assert_eq!(bench.connections.sessions()[0].recordings, 2);

    // AGT-053 — removed with the session's recordings, each part counted.
    call("record_transfer", base.clone())
        .await
        .expect("recorded");
    let removal = bench
        .recorder
        .remove_recorded_in_session(&granted.key)
        .await
        .expect("removed");
    assert_eq!((removal.removed, removal.kept), (4, 0));

    // TRF-022 — between currencies a rate is needed: none stored refuses the transfer.
    let accounts: Arc<dyn AccountServiceContract> = Arc::clone(&bench.accounts) as _;
    let assets: Arc<dyn AssetServiceContract> = Arc::clone(&bench.assets) as _;
    AccountCreationUseCase::new(accounts, assets, Arc::clone(&bench.transactions))
        .create(
            "US Broker".to_string(),
            String::new(),
            "USD".to_string(),
            false,
        )
        .await
        .expect("account");
    // The removal took the session's deposit and purchase with it.
    call(
        "record_deposit",
        json!({ "account": "PEA", "amount": 2000, "date": "2019-01-15" }),
    )
    .await
    .expect("deposited");
    call("record_purchase", json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2019-03-01" }))
        .await
        .expect("bought");
    assert_eq!(
        call("record_transfer", with(json!({ "to": "US Broker" })))
            .await
            .expect_err("refused")
            .code,
        "TransferRateMissing"
    );
    assert_eq!(
        call(
            "record_transfer",
            with(json!({ "to": "US Broker", "rate": 0 }))
        )
        .await
        .expect_err("refused")
        .code,
        "ExchangeRateNotPositive"
    );
    let converted = call(
        "record_transfer",
        with(json!({ "to": "US Broker", "rate": "1.1" })),
    )
    .await
    .expect("recorded");
    assert_eq!(converted["to_currency"], "USD");
    assert_eq!(converted["transactions"][1]["exchange_rate"], 1_100_000);
}

// AGT-047 / AGT-065A — every part of a transfer takes a place of the session's limit, checked
// before the transfer: one that would pass it is refused whole.
#[tokio::test]
async fn agt_065a_a_transfer_fits_within_the_limit() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let accounts: Arc<dyn AccountServiceContract> = Arc::clone(&bench.accounts) as _;
    let assets: Arc<dyn AssetServiceContract> = Arc::clone(&bench.assets) as _;
    AccountCreationUseCase::new(accounts, assets, Arc::clone(&bench.transactions))
        .create(
            "Wallet".to_string(),
            String::new(),
            "EUR".to_string(),
            false,
        )
        .await
        .expect("account");
    bench
        .tools
        .call(
            &granted,
            "record_deposit",
            &json!({ "account": "PEA", "amount": 2000, "date": "2019-01-15" }),
        )
        .await
        .expect("deposited");
    bench
        .tools
        .call(&granted, "record_purchase", &json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2019-03-01" }))
        .await
        .expect("bought");
    for _ in 2..super::connections::MAX_RECORDINGS - 2 {
        granted.count_recording();
    }
    let transfer = |received: i64| json!({ "from": "PEA", "to": "Wallet", "asset": "CW8", "quantity": 4, "received": received, "date": "2019-06-01" });
    assert_eq!(
        bench
            .tools
            .call(&granted, "record_transfer", &transfer(3))
            .await
            .expect_err("three parts do not fit in two places")
            .code,
        "SessionLimitReached"
    );
    let recorded = bench
        .tools
        .call(&granted, "record_transfer", &transfer(4))
        .await
        .expect("two parts fit");
    assert_eq!(recorded["transactions"].as_array().map(Vec::len), Some(2));
    assert!(!granted.may_record_one_more());
}

// AGT-049 / AGT-065A — a transfer whose parts cannot be marked is not recorded: the parts and
// their marks are one write.
#[tokio::test]
async fn agt_065a_a_transfer_that_cannot_be_marked_is_not_recorded() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    let accounts: Arc<dyn AccountServiceContract> = Arc::clone(&bench.accounts) as _;
    let assets: Arc<dyn AssetServiceContract> = Arc::clone(&bench.assets) as _;
    AccountCreationUseCase::new(accounts, assets, Arc::clone(&bench.transactions))
        .create(
            "Wallet".to_string(),
            String::new(),
            "EUR".to_string(),
            false,
        )
        .await
        .expect("account");
    for (tool, arguments) in [
        (
            "record_deposit",
            json!({ "account": "PEA", "amount": 2000, "date": "2019-01-15" }),
        ),
        (
            "record_purchase",
            json!({ "account": "PEA", "asset": "CW8", "quantity": 10, "price": 100, "date": "2019-03-01" }),
        ),
    ] {
        bench
            .tools
            .call(&granted, tool, &arguments)
            .await
            .expect("recorded");
    }
    sqlx::query("DROP TABLE agent_recordings")
        .execute(&bench.pool)
        .await
        .expect("the marks cannot be written");

    let refused = bench
        .tools
        .call(
            &granted,
            "record_transfer",
            &json!({ "from": "PEA", "to": "Wallet", "asset": "CW8", "quantity": 4, "received": 3, "date": "2019-06-01" }),
        )
        .await
        .expect_err("not marked");
    assert_eq!(refused.code, "NotRecorded");
    let parts: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions WHERE transfer_id IS NOT NULL")
            .fetch_one(&bench.pool)
            .await
            .expect("count");
    assert_eq!(parts, 0, "nothing of the transfer is written");
}

/// The first replacement request waiting.
async fn replacement_waiting(
    connections: &AgentConnections,
) -> super::connections::ReplacementRequest {
    for _ in 0..400 {
        if let Some(request) = connections.replacements().into_iter().next() {
            return request;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("no replacement request arrived");
}

/// An account of `name` in euros, created as the window creates it.
async fn euro_account(bench: &Workbench, name: &str) -> String {
    let accounts: Arc<dyn AccountServiceContract> = Arc::clone(&bench.accounts) as _;
    let assets: Arc<dyn AssetServiceContract> = Arc::clone(&bench.assets) as _;
    AccountCreationUseCase::new(accounts, assets, Arc::clone(&bench.transactions))
        .create(name.to_string(), String::new(), "EUR".to_string(), false)
        .await
        .expect("account")
        .id
}

/// Records what the owner typed, through the window's recording: the transaction recorded.
async fn typed(
    bench: &Workbench,
    recording: crate::use_cases::holding_transaction::Recording,
) -> crate::context::account::Transaction {
    *bench
        .recorder
        .record_named(recording, "2026-10-24")
        .await
        .expect("recorded")
        .transaction
}

/// What the owner typed, by kind: a trade of CW8 at `price`, or cash of `amount`.
mod owner {
    use crate::use_cases::holding_transaction::{
        CashAmount, Recording, Target, Trade, TradeAmount,
    };

    const M: i64 = 1_000_000;

    fn trade(account: &str, date: &str, quantity: i64, price: i64) -> Trade {
        Trade {
            target: Target {
                account: account.to_string(),
                asset: "CW8".to_string(),
                date: Some(date.to_string()),
                quantity: quantity * M,
            },
            amount: TradeAmount::Price(price * M),
            fees: 0,
            rate: M,
            note: None,
        }
    }

    fn cash(account: &str, date: &str, amount: i64) -> CashAmount {
        CashAmount {
            account: account.to_string(),
            date: Some(date.to_string()),
            amount: amount * M,
            note: None,
        }
    }

    pub fn buy(account: &str, date: &str, quantity: i64, price: i64) -> Recording {
        Recording::Buy(trade(account, date, quantity, price))
    }

    pub fn sell(account: &str, date: &str, quantity: i64, price: i64) -> Recording {
        Recording::Sell(trade(account, date, quantity, price))
    }

    pub fn deposit(account: &str, date: &str, amount: i64) -> Recording {
        Recording::Deposit(cash(account, date, amount))
    }

    pub fn withdraw(account: &str, date: &str, amount: i64) -> Recording {
        Recording::Withdrawal(cash(account, date, amount))
    }
}

/// An account's figures as the owner reads them, whatever ids its transactions carry: each
/// transaction's kind, date and amounts, and each holding.
async fn figures_of(bench: &Workbench, account_id: &str) -> (Vec<String>, Vec<String>) {
    let journal = bench
        .accounts
        .get_account_journal(
            account_id,
            &crate::context::account::JournalFilter::default(),
        )
        .await
        .expect("journal");
    let mut transactions: Vec<String> = journal
        .rows
        .iter()
        .map(|row| {
            let t = &row.transaction;
            format!(
                "{} {:?} {} q{} u{} f{} r{} t{} pnl{:?} cash{}",
                t.date,
                t.transaction_type,
                t.asset_id,
                t.quantity,
                t.unit_price,
                t.fees,
                t.exchange_rate,
                t.total_amount,
                t.realized_pnl,
                row.cash_balance
            )
        })
        .collect();
    transactions.sort();
    let mut holdings: Vec<String> = bench
        .accounts
        .get_holdings_for_account(account_id)
        .await
        .expect("holdings")
        .iter()
        .map(|h| {
            format!(
                "{} q{} avg{} pnl{} sold{:?}",
                h.asset_id, h.quantity, h.average_price, h.total_realized_pnl, h.last_sold_date
            )
        })
        .collect();
    holdings.sort();
    (transactions, holdings)
}

// AGT-090 / AGT-091 — a sale, a withdrawal, a deposit and a purchase forming a past transfer
// are replaced by one transfer on the owner's yes, and both accounts then stand exactly as
// if the transfer had been recorded from the start.
#[tokio::test]
async fn agt_091_four_transactions_replaced_stand_as_the_transfer_recorded_from_the_start() {
    replaced_stands_as_recorded(4).await;
}

// AGT-091 / TRF-021 — a purchase of less than was sold is a transfer with a fee in kind: the
// replacement records it as the transfer recorded so from the start does.
#[tokio::test]
async fn agt_091_less_received_than_sent_is_replaced_with_its_fee_in_kind() {
    replaced_stands_as_recorded(3).await;
}

/// The four-step way of 4 CW8 from A1 to A2, `received` of them bought again for the 600
/// withdrawn, replaced on the owner's yes, beside B1 to B2 recorded as the transfer.
async fn replaced_stands_as_recorded(received: i64) {
    use super::gate::answer_replacement;
    use crate::context::account::TransactionType;
    use crate::use_cases::transfer::TransferFigures;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let (a1, a2) = (
        euro_account(&bench, "A1").await,
        euro_account(&bench, "A2").await,
    );
    let (b1, b2) = (
        euro_account(&bench, "B1").await,
        euro_account(&bench, "B2").await,
    );
    // The same history in both pairs: 10 CW8 bought at 100.
    for account in ["A1", "B1"] {
        typed(&bench, owner::deposit(account, "2019-01-15", 2000)).await;
        typed(&bench, owner::buy(account, "2019-03-01", 10, 100)).await;
    }
    // A1 to A2 the four-step way, on four days, at 150.
    let sale = typed(&bench, owner::sell("A1", "2019-06-01", 4, 150)).await;
    let withdrawal = typed(&bench, owner::withdraw("A1", "2019-06-02", 600)).await;
    let deposit = typed(&bench, owner::deposit("A2", "2019-06-03", 600)).await;
    let purchase = typed(
        &bench,
        owner::buy("A2", "2019-06-04", received, 600 / received),
    )
    .await;
    // B1 to B2 as the transfer, from the start.
    bench
        .transfers
        .record_transfer(
            &b1,
            &b2,
            sale.asset_id.clone(),
            TransferFigures {
                date: "2019-06-01".to_string(),
                quantity_sent: 4 * M,
                quantity_received: received * M,
                exchange_rate: None,
                note: None,
            },
        )
        .await
        .expect("transferred");
    // A later sale in both receiving accounts, whose result the replacement moves.
    for account in ["A2", "B2"] {
        typed(&bench, owner::sell(account, "2019-09-01", 1, 200)).await;
    }

    let asking = {
        let (bench, granted) = (Arc::clone(&bench), granted.clone());
        // Given in any order.
        let ids = json!([purchase.id, sale.id, deposit.id, withdrawal.id]);
        tokio::spawn(async move {
            bench
                .tools
                .call(
                    &granted,
                    "request_transfer_replacement",
                    &json!({ "transactions": ids }),
                )
                .await
        })
    };
    let request = replacement_waiting(&bench.connections).await;
    let kinds: Vec<_> = request
        .transactions
        .iter()
        .map(|t| t.transaction.kind)
        .collect();
    let moved: Vec<_> = request
        .transactions
        .iter()
        .map(|t| (t.holding_moved, t.cash_moved))
        .collect();
    assert_eq!(
        moved,
        [
            (Some(-4 * M), 600 * M),
            (None, -600 * M),
            (None, 600 * M),
            (Some(received * M), -600 * M)
        ],
        "each step's quantity and cash, signed by the core"
    );
    assert_eq!(
        kinds,
        [
            TransactionType::Sell,
            TransactionType::Withdrawal,
            TransactionType::Deposit,
            TransactionType::Purchase
        ],
        "shown in the order of the steps"
    );
    assert_eq!(request.transfer.from_account, "A1");
    assert_eq!(request.transfer.to_account, "A2");
    assert_eq!(request.transfer.date, "2019-06-01", "the sale's date");
    assert_eq!(request.transfer.quantity_sent, 4 * M);
    assert_eq!(request.transfer.quantity_received, received * M);
    assert_eq!(
        request.transfer.quantity_not_received,
        (received < 4).then_some((4 - received) * M),
        "the fee in kind, when less is received"
    );
    assert_eq!(
        request.transfer.cost,
        received * 100 * M,
        "what is received, at the average price of 100 (TRF-057)"
    );
    assert_eq!(request.transfer.exchange_rate, None, "one currency");
    let (before_a1, _) = figures_of(&bench, &a1).await;
    assert!(
        before_a1.iter().any(|t| t.contains("Sell")),
        "nothing replaced before the owner's answer"
    );

    answer_replacement(&bench.connections, &bench.transfers, request.id, true)
        .await
        .expect("replaced");
    let answered = asking.await.expect("asked").expect("replaced");
    assert_eq!(answered["status"], "replaced");
    assert_eq!(answered["transfer"]["quantity_sent"], 4 * M);
    assert!(
        bench.connections.replacements().is_empty(),
        "the request is gone"
    );

    assert_eq!(figures_of(&bench, &a1).await, figures_of(&bench, &b1).await);
    assert_eq!(figures_of(&bench, &a2).await, figures_of(&bench, &b2).await);
    for id in [&sale.id, &withdrawal.id, &deposit.id, &purchase.id] {
        assert!(
            bench
                .accounts
                .get_transaction_by_id(id)
                .await
                .expect("read")
                .is_none(),
            "the four are removed"
        );
    }
    // AGT-045 — the transfer is the owner's: no part carries an agent's mark.
    let journal = bench
        .accounts
        .get_account_journal(&a2, &crate::context::account::JournalFilter::default())
        .await
        .expect("journal");
    let parts: Vec<_> = journal.rows.iter().map(|row| &row.transaction).collect();
    assert!(
        parts.iter().any(|t| t.transfer_id.is_some()),
        "the transfer is in A2"
    );
    for part in parts.iter().filter(|t| t.transfer_id.is_some()) {
        assert!(
            bench
                .accounts
                .agent_recording_of(&part.id)
                .await
                .expect("read")
                .is_none(),
            "unmarked"
        );
    }
}

// AGT-090 — what is not four transactions the owner typed forming a transfer, or what the
// window's transfer would refuse on the accounts without them, is refused before asking.
#[tokio::test]
async fn agt_090_what_is_no_transfer_is_refused_before_asking() {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = workbench(data_dir.path()).await;
    let granted = session(&bench.connections).await;
    euro_account(&bench, "A1").await;
    euro_account(&bench, "A2").await;
    typed(&bench, owner::deposit("A1", "2019-01-15", 2000)).await;
    typed(&bench, owner::buy("A1", "2019-03-01", 10, 100)).await;
    let sale = typed(&bench, owner::sell("A1", "2019-06-01", 4, 150)).await;
    let withdrawal = typed(&bench, owner::withdraw("A1", "2019-06-02", 600)).await;
    // More is deposited than the purchase took, and spent later: without the deposit the
    // cash of A2 cannot pay it.
    let deposit = typed(&bench, owner::deposit("A2", "2019-06-03", 700)).await;
    let purchase = typed(&bench, owner::buy("A2", "2019-06-04", 4, 150)).await;
    let spent = typed(&bench, owner::withdraw("A2", "2019-07-01", 100)).await;
    let other_sale = typed(&bench, owner::sell("A1", "2019-08-01", 1, 150)).await;
    let marked = bench
        .tools
        .call(
            &granted,
            "record_deposit",
            &json!({ "account": "A1", "amount": 5, "date": "2019-09-01" }),
        )
        .await
        .expect("recorded");
    let marked = recorded_id(&marked);

    let four = |ids: [&String; 4]| json!({ "transactions": ids });
    for (arguments, code) in [
        (json!({}), "MissingArgument"),
        (json!({ "transactions": [1, 2, 3, 4] }), "MissingArgument"),
        (
            json!({ "transactions": [sale.id, withdrawal.id, deposit.id] }),
            "InvalidArguments",
        ),
        (
            four([&sale.id, &sale.id, &deposit.id, &purchase.id]),
            "InvalidArguments",
        ),
        // AGT-048A — an id is bounded as any text an agent sends.
        (
            json!({ "transactions": [sale.id, withdrawal.id, deposit.id, "x".repeat(201)] }),
            "InvalidArguments",
        ),
        (
            json!({ "transactions": [sale.id, withdrawal.id, deposit.id, "no-such-id"] }),
            "TransactionNotFound",
        ),
        (
            four([&sale.id, &marked, &deposit.id, &purchase.id]),
            "NotTypedByOwner",
        ),
        (
            four([&sale.id, &other_sale.id, &deposit.id, &purchase.id]),
            "NotATransfer",
        ),
        // The withdrawal is not in the sale's account.
        (
            four([&sale.id, &spent.id, &deposit.id, &purchase.id]),
            "NotATransfer",
        ),
        (
            json!({ "transactions": [sale.id, withdrawal.id, deposit.id, purchase.id], "rate": 2 }),
            "TransferRateNotOne",
        ),
        (
            four([&sale.id, &withdrawal.id, &deposit.id, &purchase.id]),
            "InsufficientCash",
        ),
    ] {
        let refused = bench
            .tools
            .call(&granted, "request_transfer_replacement", &arguments)
            .await
            .expect_err("refused");
        assert_eq!(refused.code, code, "{arguments}");
    }
    assert!(
        bench.connections.replacements().is_empty(),
        "nothing was asked"
    );
    assert!(
        bench
            .accounts
            .get_transaction_by_id(&sale.id)
            .await
            .expect("read")
            .is_some(),
        "nothing was written"
    );
}

// AGT-091 — a refusal replaces nothing; a yes on four that changed since the owner was shown
// them replaces nothing and says so; an answer to a request no longer waiting is refused;
// a session that ends withdraws its request.
#[tokio::test]
async fn agt_091_a_refusal_or_a_change_since_replaces_nothing() {
    use super::gate::{answer_replacement, AnswerRefused};

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let a2 = {
        euro_account(&bench, "A1").await;
        euro_account(&bench, "A2").await
    };
    typed(&bench, owner::deposit("A1", "2019-01-15", 2000)).await;
    typed(&bench, owner::buy("A1", "2019-03-01", 10, 100)).await;
    let sale = typed(&bench, owner::sell("A1", "2019-06-01", 4, 150)).await;
    let withdrawal = typed(&bench, owner::withdraw("A1", "2019-06-02", 600)).await;
    let deposit = typed(&bench, owner::deposit("A2", "2019-06-03", 600)).await;
    let purchase = typed(&bench, owner::buy("A2", "2019-06-04", 4, 150)).await;
    let ids = json!({ "transactions": [sale.id, withdrawal.id, deposit.id, purchase.id] });
    let ask = || {
        let (bench, granted, ids) = (Arc::clone(&bench), granted.clone(), ids.clone());
        tokio::spawn(async move {
            bench
                .tools
                .call(&granted, "request_transfer_replacement", &ids)
                .await
        })
    };
    let before = figures_of(&bench, &a2).await;

    // Refused: nothing replaced, and the agent is told nothing else.
    let asked = ask();
    let request = replacement_waiting(&bench.connections).await;
    answer_replacement(&bench.connections, &bench.transfers, request.id, false)
        .await
        .expect("answered");
    let refused = asked.await.expect("asked").expect_err("refused");
    assert_eq!(refused.code, "ReplacementRefused");
    assert_eq!(figures_of(&bench, &a2).await, before);
    assert_eq!(
        answer_replacement(&bench.connections, &bench.transfers, request.id, true).await,
        Err(AnswerRefused::Gone),
        "a request no longer waiting"
    );

    // Changed since the owner was shown it: the yes replaces nothing.
    let asked = ask();
    let request = replacement_waiting(&bench.connections).await;
    bench
        .accounts
        .correct_transaction(
            &deposit.account_id,
            &deposit.id,
            deposit.date.clone(),
            deposit.quantity,
            deposit.unit_price,
            deposit.exchange_rate,
            deposit.fees,
            Some(deposit.total_amount),
            Some("typed again".to_string()),
        )
        .await
        .expect("corrected");
    assert_eq!(
        answer_replacement(&bench.connections, &bench.transfers, request.id, true).await,
        Err(AnswerRefused::NotDone {
            reason: "TransactionChanged".to_string()
        })
    );
    assert_eq!(
        asked.await.expect("asked").expect_err("not replaced").code,
        "TransactionChanged"
    );
    assert!(
        bench
            .accounts
            .get_transaction_by_id(&sale.id)
            .await
            .expect("read")
            .is_some(),
        "nothing was written"
    );

    // AGT-082 — a session that ends withdraws its request: its call is refused.
    let asked = ask();
    replacement_waiting(&bench.connections).await;
    assert!(bench.connections.end(granted.session_id));
    assert_eq!(
        asked.await.expect("asked").expect_err("refused").code,
        "ReplacementRefused"
    );
    assert!(bench.connections.replacements().is_empty());

    // Deleted since the owner was shown it: the yes replaces nothing.
    let granted = session(&bench.connections).await;
    let asked = {
        let (bench, ids) = (Arc::clone(&bench), ids.clone());
        tokio::spawn(async move {
            bench
                .tools
                .call(&granted, "request_transfer_replacement", &ids)
                .await
        })
    };
    let request = replacement_waiting(&bench.connections).await;
    bench
        .accounts
        .cancel_transaction(&withdrawal.account_id, &withdrawal.id)
        .await
        .expect("deleted");
    assert_eq!(
        answer_replacement(&bench.connections, &bench.transfers, request.id, true).await,
        Err(AnswerRefused::NotDone {
            reason: "TransactionNotFound".to_string()
        })
    );
    assert_eq!(
        asked.await.expect("asked").expect_err("not replaced").code,
        "TransactionNotFound"
    );
    assert!(
        bench
            .accounts
            .get_transaction_by_id(&sale.id)
            .await
            .expect("read")
            .is_some(),
        "nothing was written"
    );
}

// AGT-080 — a part of a transfer the owner typed is refused before anything is asked: the
// window's deletion would remove the whole transfer, more than the dialog shows (TRF-040).
#[tokio::test]
async fn agt_080_a_part_of_a_transfer_is_refused_before_asking() {
    use crate::use_cases::transfer::TransferFigures;

    let data_dir = tempfile::tempdir().expect("tempdir");
    let bench = Arc::new(workbench(data_dir.path()).await);
    let granted = session(&bench.connections).await;
    let (from, to) = (
        euro_account(&bench, "A1").await,
        euro_account(&bench, "A2").await,
    );
    typed(&bench, owner::deposit("A1", "2019-01-15", 2000)).await;
    let bought = typed(&bench, owner::buy("A1", "2019-03-01", 10, 100)).await;
    // 4 sent, 3 received: a transfer out, a transfer in and a fee in kind.
    let parts = bench
        .transfers
        .record_transfer(
            &from,
            &to,
            bought.asset_id.clone(),
            TransferFigures {
                date: "2019-06-01".to_string(),
                quantity_sent: 4 * M,
                quantity_received: 3 * M,
                exchange_rate: None,
                note: None,
            },
        )
        .await
        .expect("transferred");
    assert_eq!(parts.len(), 3, "the transfer's three parts");

    for part in &parts {
        // Refused at once: a call that waits for the owner has asked.
        let refused = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            bench.tools.call(
                &granted,
                "request_removal",
                &json!({ "transaction": part.id }),
            ),
        )
        .await
        .expect("refused before asking, not waiting for the owner")
        .expect_err("refused");
        assert_eq!(
            refused.code, "PartOfTransfer",
            "{:?}",
            part.transaction_type
        );
    }
    assert!(bench.connections.removals().is_empty(), "nothing was asked");
}
