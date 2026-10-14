//! The agent bridge as an agent client meets it (AGT-010, AGT-011, AGT-046): the program
//! itself, started with `--mcp` as its own process and spoken to over its standard input
//! and output. The other tests of the agent connection call the bridge's code; this one
//! runs what an agent client is told to start.
#![cfg(all(feature = "app", unix))]

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

/// Starts the program with `--mcp` over an empty data folder, sends `messages` one per
/// line, closes its input and returns what it answered, one value per line.
fn answers_of_the_bridge(messages: &[Value]) -> Vec<Value> {
    let data_dir = tempfile::tempdir().expect("tempdir");
    let mut bridge = Command::new(env!("CARGO_BIN_EXE_folioneer"))
        .arg("--mcp")
        // No application runs over this folder: the bridge reaches none.
        .env("XDG_DATA_HOME", data_dir.path())
        .env("HOME", data_dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the program starts with --mcp");
    {
        let mut input = bridge.stdin.take().expect("its input");
        for message in messages {
            writeln!(input, "{message}").expect("a message is sent");
        }
    }
    let output = bridge.wait_with_output().expect("the bridge ends");
    assert!(
        output.status.success(),
        "the bridge ends cleanly when its input closes"
    );
    String::from_utf8(output.stdout)
        .expect("text")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is one JSON message"))
        .collect()
}

// AGT-010 / AGT-046 — the program started with `--mcp` introduces itself, lists the tools
// an agent may call — each with its description and the schema of its arguments — and
// answers nothing to a notification.
#[test]
fn agt_010_the_program_started_with_mcp_lists_its_tools() {
    let answers = answers_of_the_bridge(&[
        json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": { "name": "claude-code", "version": "test" } } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    ]);

    assert_eq!(answers.len(), 2, "a notification is not answered");
    assert_eq!(answers[0]["id"], 0);
    assert_eq!(answers[0]["result"]["serverInfo"]["name"], "folioneer");
    let tools = answers[1]["result"]["tools"].as_array().expect("tools");
    let names: Vec<&str> = tools
        .iter()
        .map(|tool| tool["name"].as_str().expect("a name"))
        .collect();
    assert_eq!(
        names,
        [
            "portfolio_summary",
            "list_accounts",
            "list_assets",
            "list_holdings",
            "list_transactions",
            "get_price",
            "record_opening_balance",
            "record_purchase",
            "record_sale",
            "record_deposit",
            "record_withdrawal",
            "record_dividend",
            "record_split",
            "record_free_shares",
            "record_price",
            "add_asset",
            "correct_recording",
            "cancel_recording",
            "request_removal",
        ]
    );
    for tool in tools {
        let name = tool["name"].as_str().expect("a name");
        assert!(
            tool["description"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "{name} says what it does"
        );
        assert_eq!(tool["inputSchema"]["type"], "object", "{name}");
    }
}

// AGT-013 — the program started with `--mcp` gives each tool a title and says whether it
// only reads: the read tools do, and everything that records, corrects, cancels or asks
// to remove says it does not — so an agent client asks its user before a recording and not before a
// read.
#[test]
fn agt_013_every_tool_listed_says_whether_it_only_reads() {
    let answers = answers_of_the_bridge(&[
        json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": { "name": "claude-code", "version": "test" } } }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
    ]);
    let tools = answers[1]["result"]["tools"].as_array().expect("tools");
    assert!(!tools.is_empty());

    let mut reading = Vec::new();
    let mut titles = std::collections::BTreeSet::new();
    for tool in tools {
        let name = tool["name"].as_str().expect("a name");
        let title = tool["title"].as_str().unwrap_or_default();
        assert!(!title.is_empty(), "{name} has a title");
        assert!(titles.insert(title), "{name} shares its title \"{title}\"");
        let only_reads = tool["annotations"]["readOnlyHint"]
            .as_bool()
            .unwrap_or_else(|| panic!("{name} says whether it only reads"));
        let records = name.starts_with("record_")
            || name == "correct_recording"
            || name == "cancel_recording"
            || name == "request_removal"
            || name == "add_asset";
        assert_eq!(only_reads, !records, "{name}");
        if only_reads {
            reading.push(name);
        }
    }
    assert_eq!(
        reading,
        [
            "portfolio_summary",
            "list_accounts",
            "list_assets",
            "list_holdings",
            "list_transactions",
            "get_price"
        ]
    );
}

// AGT-011 — with no application open over its data folder, a tool call is answered that
// Folioneer is not open, an unknown tool is an error, and nothing is written.
#[test]
fn agt_011_the_program_started_with_mcp_reaches_no_closed_application() {
    let answers = answers_of_the_bridge(&[
        json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {
            "clientInfo": { "name": "claude-code" } } }),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {
            "name": "record_price",
            "arguments": { "asset": "CW8", "price": "512.30", "date": "2019-12-31" } } }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
            "name": "no_such_tool", "arguments": {} } }),
    ]);

    assert_eq!(answers.len(), 3);
    let refused = &answers[1]["result"];
    assert_eq!(refused["isError"], true);
    assert!(refused["content"][0]["text"]
        .as_str()
        .expect("a reason")
        .starts_with("Folioneer is not open, or agents are not allowed to connect"));
    assert_eq!(answers[2]["error"]["message"], "no such tool");
}
