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
        ToolDefinition {
            name: "record_opening_balance",
            description: "Record what an account already held of an asset on a date: the quantity and its total cost. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "quantity": { "type": ["string", "number"], "description": "How many units were held." },
                    "total_cost": { "type": ["string", "number"], "description": "What they cost in all, in the account's currency." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." }
                },
                "required": ["account", "asset", "quantity", "total_cost"],
                "additionalProperties": false
            }),
        },
        trade_definition(
            "record_purchase",
            "Record a purchase of an asset in an account. Give either the unit price, in the asset's currency, or the broker's all-in total, in the account's currency. The cash of the account pays for it. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
        ),
        trade_definition(
            "record_sale",
            "Record a sale of an asset held in an account. Give either the unit price, in the asset's currency, or the broker's all-in total, in the account's currency. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
        ),
        ToolDefinition {
            name: "correct_recording",
            description: "Correct a transaction this session recorded: give its id, as the recording returned it, and only what changes. A transaction the owner typed, or one another session recorded, is refused. Figures are decimals with a dot (12.5), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "transaction": { "type": "string", "description": "The transaction's id, as the recording returned it." },
                    "quantity": { "type": ["string", "number"], "description": "The right quantity." },
                    "price": { "type": ["string", "number"], "description": "The right unit price, in the asset's currency. Not with total." },
                    "total": { "type": ["string", "number"], "description": "The right all-in amount, in the account's currency; for an opening balance, its total cost. Not with price." },
                    "fees": { "type": ["string", "number"], "description": "The right fees, in the account's currency." },
                    "rate": { "type": ["string", "number"], "description": "The right exchange rate from the asset's currency to the account's." },
                    "date": { "type": "string", "description": "The right date, YYYY-MM-DD, today or earlier." },
                    "note": { "type": "string", "description": "The right note." }
                },
                "required": ["transaction"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "cancel_recording",
            description: "Cancel a transaction this session recorded: give its id, as the recording returned it. A transaction the owner typed, or one another session recorded, is refused.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "transaction": { "type": "string", "description": "The transaction's id, as the recording returned it." }
                },
                "required": ["transaction"],
                "additionalProperties": false
            }),
        },
    ]
}

/// A purchase or a sale as an agent is told of it.
fn trade_definition(name: &'static str, description: &'static str) -> ToolDefinition {
    ToolDefinition {
        name,
        description,
        input_schema: json!({
            "type": "object",
            "properties": {
                "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                "quantity": { "type": ["string", "number"], "description": "How many units." },
                "price": { "type": ["string", "number"], "description": "Unit price, in the asset's currency. Not with total." },
                "total": { "type": ["string", "number"], "description": "The broker's all-in amount, in the account's currency. Not with price." },
                "fees": { "type": ["string", "number"], "description": "Fees, in the account's currency. Default: 0." },
                "rate": { "type": ["string", "number"], "description": "Exchange rate from the asset's currency to the account's. Default: 1." },
                "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                "note": { "type": "string", "description": "A note kept with the transaction." }
            },
            "required": ["account", "asset", "quantity"],
            "additionalProperties": false
        }),
    }
}
