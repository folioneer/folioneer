//! The tools an agent is told of (AGT-040): fixed text, which the bridge lists on its own.
//! Running a tool is the running application's (`tools.rs`); nothing here reaches it.

use serde_json::{json, Value};

/// A tool as an agent client is told of it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    /// The tool's name.
    pub name: &'static str,
    /// What the tool does, for the agent to read.
    pub description: &'static str,
    /// The JSON schema of its arguments.
    pub input_schema: Value,
}

/// AGT-040 — the tools offered, in the order they are listed.
pub fn definitions() -> Vec<ToolDefinition> {
    let nothing = || json!({ "type": "object", "properties": {}, "additionalProperties": false });
    vec![
        ToolDefinition {
            name: "portfolio_summary",
            description: "The value of the portfolio: each account with its total value, its unrealized gain and its performance this year, and the total over the accounts. Amounts and quantities are whole numbers of millionths: 1500000 is 1.5.",
            input_schema: nothing(),
        },
        ToolDefinition {
            name: "list_accounts",
            description: "The accounts, by name, each with its currency. An account is named by its name in the other tools.",
            input_schema: nothing(),
        },
        ToolDefinition {
            name: "list_assets",
            description: "The assets a holding can be of, by name: reference, kind, class, currency, ISIN and exchange code. Cash is not listed.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "archived": { "type": "boolean", "description": "Include archived assets. Default: false." }
                },
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "list_holdings",
            description: "The holdings of one account, with their quantities, costs, latest prices and gains, and the account's totals; as they stood on a past date when `as_of` is given. Amounts and quantities are whole numbers of millionths: 1500000 is 1.5.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "as_of": { "type": "string", "description": "A past date, YYYY-MM-DD. Default: today." }
                },
                "required": ["account"],
                "additionalProperties": false
            }),
        },
    ]
}
