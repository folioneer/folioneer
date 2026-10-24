//! The tools an agent is told of (AGT-046): fixed text, which the bridge lists on its own.
//! Running a tool is the running application's (`tools.rs`); nothing here reaches it.

use serde_json::{json, Value};

/// A tool as an agent client is told of it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDefinition {
    /// The tool's name.
    pub name: &'static str,
    /// A few words a person reads in the agent client (AGT-013).
    pub title: &'static str,
    /// Whether it only reads the portfolio: what the client is told as `readOnlyHint`,
    /// and what counts as a read of the session (AGT-013, AGT-052).
    pub only_reads: bool,
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
            title: "Portfolio summary",
            only_reads: true,
            description: "The value of the portfolio: each account with its total value, its unrealized gain, its performance this year and how many of its holdings have an outdated price (outdated_price_count), and the total over the accounts. Amounts and quantities are whole numbers of millionths: 1500000 is 1.5.",
            input_schema: nothing(),
        },
        ToolDefinition {
            name: "list_accounts",
            title: "List accounts",
            only_reads: true,
            description: "The accounts, by name, each with its currency, its country (two letters) and the identifier of its tax wrapper, or null. An account is named by its name in the other tools.",
            input_schema: nothing(),
        },
        ToolDefinition {
            name: "list_assets",
            title: "List assets",
            only_reads: true,
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
            title: "List an account's holdings",
            only_reads: true,
            description: "The holdings of one account, with their quantities, costs, latest prices and gains, and the account's totals — total_capital_brought_in is what the owner brought in, cash and opening balances at cost; price_outdated says a holding's latest price is too old to trust for its kind (5 business days, 10 for a fund, never for a custom asset), and outdated_price_count how many are; unrecorded_splits lists the splits the price provider reports that this account held through without recording one on that date, each with its date and its factor as new for old — record it with record_split, nothing is recorded automatically; as they stood on a past date when `as_of` is given. Amounts and quantities are whole numbers of millionths: 1500000 is 1.5.",
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
            name: "list_transactions",
            title: "List an account's transactions",
            only_reads: true,
            description: "The transactions of one account, oldest first, 100 to a page: each with its id — the one correct_recording and cancel_recording take — its date, transaction_type, asset, quantity, unit price, total, fees, exchange rate and note, and who recorded it (recorded_by.by): owner, agent — with the agent's name — or session, this session. owner means no agent recorded it on this computer. Read it before recording from a statement, so that nothing is recorded twice. Amounts and quantities are whole numbers of millionths: 1500000 is 1.5; a figure a transaction does not have is 0, and the quantity of a Split is its factor.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "Only this asset's transactions: its name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "from": { "type": "string", "description": "Only from this date on, YYYY-MM-DD." },
                    "to": { "type": "string", "description": "Only up to this date, YYYY-MM-DD." },
                    "transaction_type": { "type": "string", "description": "Only this type: Purchase, Sell, OpeningBalance, Deposit, Withdrawal, Dividend, FreeShares, ManagementFee, Split, Interest or WriteOff." },
                    "recorded_by": { "type": "string", "description": "Only what the owner typed (owner), what an agent recorded (agent), or what this session recorded (session)." },
                    "page": { "type": "integer", "description": "The page, from 1. Default: 1." }
                },
                "required": ["account"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "get_price",
            title: "Read an asset's price at a date",
            only_reads: true,
            description: "The price the application holds for one unit of an asset at a date, in the asset's currency: the price of that date or, when it has none, the closest earlier one — its own date says which. With it, its source: Manual (typed by the owner), Agent (recorded by an agent), or the name of the provider it was fetched from. status is no_price when the asset has no price up to that date. reported_splits lists every split the price provider reports for the asset, oldest first, each with its date and its factor as new for old (20 for 1 is a 20-for-1 split); compare them with the splits recorded in the account. Use it to check a valuation against a statement. The price is a whole number of millionths: 1500000 is 1.5.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." }
                },
                "required": ["asset"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "record_opening_balance",
            title: "Record an opening balance",
            only_reads: false,
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
            "Record a purchase",
            "Record a purchase of an asset in an account. Give either the unit price, in the asset's currency, or the broker's all-in total, in the account's currency. The cash of the account pays for it. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
        ),
        trade_definition(
            "record_sale",
            "Record a sale",
            "Record a sale of an asset held in an account. Give either the unit price, in the asset's currency, or the broker's all-in total, in the account's currency. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
        ),
        cash_definition(
            "record_deposit",
            "Record a deposit",
            "Record cash paid into an account from outside. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
        ),
        cash_definition(
            "record_withdrawal",
            "Record a withdrawal",
            "Record cash taken out of an account. The account must hold that cash on the date. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
        ),
        ToolDefinition {
            name: "record_dividend",
            title: "Record a dividend",
            only_reads: false,
            description: "Record a dividend an asset held in an account paid: the cash of the account receives it, the holding is unchanged. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "The paying asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "amount": { "type": ["string", "number"], "description": "The total net amount received, in the asset's currency." },
                    "rate": { "type": ["string", "number"], "description": "Exchange rate from the asset's currency to the account's. Default: 1." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                    "note": { "type": "string", "description": "A note kept with the transaction." }
                },
                "required": ["account", "asset", "amount"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "record_split",
            title: "Record a split",
            only_reads: false,
            description: "Record a stock split, or a reverse split, of an asset held in an account: `new` shares for every `old` shares held (2 and 1 for a 2-for-1 split, 1 and 10 for a 1-for-10 reverse split). The quantity held is rescaled from that date, its cost is unchanged and no cash moves. It records no price: record the price after the split with record_price, or the price from before it values the new quantity. The recording follows the application's rules and is marked as recorded by this agent.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "new": { "type": ["string", "integer"], "description": "Shares after the split, for every `old` shares before: a whole number above zero." },
                    "old": { "type": ["string", "integer"], "description": "Shares before the split: a whole number above zero." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                    "note": { "type": "string", "description": "A note kept with the transaction." }
                },
                "required": ["account", "asset", "new", "old"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "record_free_shares",
            title: "Record free shares",
            only_reads: false,
            description: "Record shares of an asset held in an account received at no cost — a bonus issue, a dividend paid in shares: the quantity held rises, its cost is unchanged and no cash moves. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "quantity": { "type": ["string", "number"], "description": "How many shares were received." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                    "note": { "type": "string", "description": "A note kept with the transaction." }
                },
                "required": ["account", "asset", "quantity"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "record_write_off",
            title: "Record a write-off",
            only_reads: false,
            description: "Record a quantity of an asset held in an account that left without proceeds — a network fee paid in the asset, dust, coins sent away or lost: the quantity held falls at its average cost, the cost taken out is a realized loss and no cash moves. The recording follows the application's rules and is marked as recorded by this agent. Figures are decimals with a dot (12.5), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "quantity": { "type": ["string", "number"], "description": "How much of the asset left." },
                    "reason": { "type": "string", "enum": ["NetworkFee", "Dust", "Lost", "Other"], "description": "Why it left: NetworkFee (a fee paid in the asset), Dust (a remainder too small to use), Lost (sent away or lost for good), Other." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                    "note": { "type": "string", "description": "A note kept with the transaction." }
                },
                "required": ["account", "asset", "quantity", "reason"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "record_transfer",
            title: "Record a transfer",
            only_reads: false,
            description: "Record an asset moved from one account of the portfolio to another — shares moved between brokers, coins sent from an exchange to a wallet: the quantity leaves the sending account at its average cost and enters the receiving one at that cost, converted at the rate between the two accounts' currencies; no cash moves and no gain is realized. When less arrives than was sent, the difference is a fee in kind, written off in the sending account. The transfer's parts follow the application's rules and are marked as recorded by this agent; cancel_recording on any of them removes the whole transfer, and a part is not corrected: cancel it and record the transfer again. Figures are decimals with a dot (12.5), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from": { "type": "string", "description": "The sending account's name, as list_accounts gives it." },
                    "to": { "type": "string", "description": "The receiving account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "quantity": { "type": ["string", "number"], "description": "How much of the asset left the sending account." },
                    "received": { "type": ["string", "number"], "description": "How much reached the receiving account, not above quantity. Default: quantity." },
                    "rate": { "type": ["string", "number"], "description": "Units of the receiving account's currency for one unit of the sending account's. Default: the rate stored for that date; give none between accounts of one currency." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                    "note": { "type": "string", "description": "A note kept with each part of the transfer." }
                },
                "required": ["from", "to", "asset", "quantity"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "record_interest",
            title: "Record interest",
            only_reads: false,
            description: "Record interest credited in an account: in units to an interest-bearing asset held there, or, with no asset, to the account's cash in its currency. Give exactly one of percent, the share of what is held on the date that is credited, and quantity, what is credited. The recording follows the application's rules and is marked as recorded by this agent; it is not previewed. Figures are decimals with a dot (2.5 for 2.5 %), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared. Leave it out for interest on the account's cash." },
                    "percent": { "type": ["string", "number"], "description": "The percent of what is held on the date that is credited, 2.5 for 2.5 %." },
                    "quantity": { "type": ["string", "number"], "description": "What is credited: units of the asset, or an amount in the account's currency for its cash." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                    "note": { "type": "string", "description": "A note kept with the transaction." }
                },
                "required": ["account"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "record_management_fee",
            title: "Record a management fee",
            only_reads: false,
            description: "Record a management fee taken in units of an asset held in an account, as an insurance contract takes it: the quantity held falls, its cost stays and no cash moves. Give exactly one of percent, the share of what is held on the date that is removed, and resulting_quantity, what the holding holds after the fee. A fee the holding's fee schedule already generated for that period is refused: ask to change that one with request_change instead. The recording follows the application's rules and is marked as recorded by this agent; it is not previewed. Figures are decimals with a dot (0.5 for 0.5 %), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "percent": { "type": ["string", "number"], "description": "The percent of what is held on the date that is removed, 0.5 for 0.5 %." },
                    "resulting_quantity": { "type": ["string", "number"], "description": "What the holding holds after the fee." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                    "note": { "type": "string", "description": "A note kept with the transaction." }
                },
                "required": ["account", "asset"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "record_price",
            title: "Record a price",
            only_reads: false,
            description: "Record the price of one unit of an asset on a date, in the asset's currency — a year-end price from a statement, for instance. It is stored as recorded by an agent and replaces the price the asset already has on that date, if any. Figures are decimals with a dot (12.5), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "asset": { "type": "string", "description": "The asset's name, reference or ISIN, or REFERENCE@EXCHANGE when a reference is shared." },
                    "price": { "type": ["string", "number"], "description": "The price of one unit, in the asset's currency, with at most six decimals." },
                    "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." }
                },
                "required": ["asset", "price"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "add_asset",
            title: "Add an asset",
            only_reads: false,
            description: "Add an asset the portfolio does not have yet, so that it can be recorded in an account: a listed one (a share, a fund, a bond — it has an ISIN), a crypto asset, or a custom one (anything else: real estate, an unlisted fund). Look for it with list_assets first: the same asset is refused twice. Give a kind, a class, or both. It takes one place of the session's limit.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "The asset's name." },
                    "reference": { "type": "string", "description": "Its ticker, its symbol for a crypto asset, or a short reference of your choice for a custom one." },
                    "currency": { "type": "string", "description": "The ISO code of the currency it is priced in: EUR, USD." },
                    "kind": { "type": "string", "description": "Listed, Crypto or Custom. Left out, it follows from the class and the ISIN." },
                    "class": { "type": "string", "description": "RealEstate, Stocks, Bonds, ETF, ETP, MutualFunds, DigitalAsset or Derivatives. Left out, the usual class of the kind: Stocks for Listed, DigitalAsset for Crypto, RealEstate for Custom." },
                    "isin": { "type": "string", "description": "Its ISIN: required for a listed asset, refused for the others." },
                    "exchange": { "type": "string", "description": "The MIC code of its exchange (XPAR), for a listed asset. Optional." }
                },
                "required": ["name", "reference", "currency"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "add_account",
            title: "Create an account",
            only_reads: false,
            description: "Create an account the portfolio does not have yet — a brokerage account, a savings plan, a crypto wallet — so that it can be recorded in, as the window creates one: its cash line is created with it and its management fees are off. Look for it with list_accounts first: a name another account has is refused, case ignored. It is usable at once by its name and takes one place of the session's limit.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "The account's name, as the owner will see it. At most 100 characters." },
                    "currency": { "type": "string", "description": "The ISO code of its currency: EUR, USD." },
                    "bank": { "type": "string", "description": "The bank or broker that holds it. Optional; at most 100 characters." },
                    "country": { "type": "string", "description": "Its country, two upper-case letters (FR, DE), when it is not the portfolio's. Optional." },
                    "wrapper": { "type": "string", "description": "The identifier of its tax wrapper (fr_pea, brokerage), as list_accounts gives them. Optional." }
                },
                "required": ["name", "currency"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "correct_recording",
            title: "Correct a recording",
            only_reads: false,
            description: "Correct a transaction this session recorded, of any type, as the window corrects it: give its id, as the recording returned it, and only what changes; the rest stays. Every type takes date and note; a purchase or a sale quantity, price or total (not both), fees and rate; an opening balance quantity and total (its total cost); a deposit or a withdrawal amount; a dividend amount (in the asset's currency) and rate; a split new and old, its ratio; free shares, interest or a management fee quantity; a write-off quantity and reason. A transaction the owner typed, or one another session recorded, is refused. Figures are decimals with a dot (12.5), not millionths.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "transaction": { "type": "string", "description": "The transaction's id, as the recording returned it." },
                    "quantity": { "type": ["string", "number"], "description": "The right quantity." },
                    "price": { "type": ["string", "number"], "description": "The right unit price, in the asset's currency. Not with total." },
                    "total": { "type": ["string", "number"], "description": "The right all-in amount, in the account's currency; for an opening balance, its total cost. Not with price." },
                    "fees": { "type": ["string", "number"], "description": "The right fees, in the account's currency." },
                    "rate": { "type": ["string", "number"], "description": "The right exchange rate from the asset's currency to the account's." },
                    "amount": { "type": ["string", "number"], "description": "The right amount of a deposit, a withdrawal or a dividend." },
                    "new": { "type": ["string", "integer"], "description": "A split's shares after, for old shares before." },
                    "old": { "type": ["string", "integer"], "description": "A split's shares before." },
                    "reason": { "type": "string", "enum": ["NetworkFee", "Dust", "Lost", "Other"], "description": "A write-off's right reason." },
                    "date": { "type": "string", "description": "The right date, YYYY-MM-DD, today or earlier." },
                    "note": { "type": "string", "description": "The right note." }
                },
                "required": ["transaction"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "cancel_recording",
            title: "Cancel a recording",
            only_reads: false,
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
        ToolDefinition {
            name: "preview_recording",
            title: "Preview a recording",
            only_reads: true,
            description: "Preview what a recording, a correction or a price would do, writing nothing: give tool, one of record_purchase, record_sale, record_opening_balance, record_split, record_deposit, record_withdrawal, record_dividend, record_free_shares, record_write_off, record_transfer, record_price, correct_recording, and arguments, the object that tool takes. Answered with the refusal the real call would give, or with the transaction as it would be recorded and the account on the evening of its date without it (before) and with it (after): each asset held, by reference and name, with its quantity and average price, and the cash. A transfer is answered with its parts and both accounts, from and to, each before and after. A price is answered with the price it would replace at that date, or null. Read the account today with list_holdings once it is recorded.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "tool": { "type": "string", "description": "The recording tool to preview." },
                    "arguments": { "type": "object", "description": "The arguments that tool takes." }
                },
                "required": ["tool", "arguments"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "request_removal",
            title: "Ask to remove a transaction you typed",
            only_reads: false,
            description: "Ask the owner to remove a transaction the owner typed: give its id, as list_transactions gives it. The owner is shown it in the window and answers it alone; the call waits for that answer. Answered removed, or refused with RemovalRefused, which gives nothing else. A transaction an agent recorded is refused: a session cancels its own with cancel_recording.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "transaction": { "type": "string", "description": "The transaction's id, as list_transactions gives it." }
                },
                "required": ["transaction"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "request_change",
            title: "Ask to change a transaction you typed",
            only_reads: false,
            description: "Ask the owner to change a transaction the owner typed: give its id, as list_transactions gives it, and what changes; the rest stays. Every type takes date and note; a purchase or a sale quantity, price or total (not both), fees and rate; an opening balance quantity and total (its total cost); a deposit or a withdrawal amount; a dividend amount (in the asset's currency) and rate; a split new and old, its ratio; free shares, interest, a management fee or a write-off quantity. A change the window's correction would refuse is refused before asking. The owner is shown it as it is and as it would become, and answers it alone; the call waits for that answer. Answered changed, with the transaction as it now stands, or refused with ChangeRefused, which gives nothing else. A transaction an agent recorded is refused: a session corrects its own with correct_recording.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "transaction": { "type": "string", "description": "The transaction's id, as list_transactions gives it." },
                    "date": { "type": "string", "description": "The new date, YYYY-MM-DD." },
                    "quantity": { "type": ["string", "number"], "description": "The new quantity." },
                    "price": { "type": ["string", "number"], "description": "The new unit price, in the asset's currency." },
                    "total": { "type": ["string", "number"], "description": "The new all-in total, in the account's currency; an opening balance's total cost." },
                    "fees": { "type": ["string", "number"], "description": "The new fees." },
                    "rate": { "type": ["string", "number"], "description": "The new exchange rate." },
                    "amount": { "type": ["string", "number"], "description": "The new amount of a deposit, a withdrawal or a dividend." },
                    "new": { "type": ["string", "integer"], "description": "A split's shares after, for old shares before." },
                    "old": { "type": ["string", "integer"], "description": "A split's shares before." },
                    "note": { "type": "string", "description": "The new note." }
                },
                "required": ["transaction"],
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "request_transfer_replacement",
            title: "Ask to replace four transactions you typed with a transfer",
            only_reads: false,
            description: "Ask the owner to replace four transactions the owner typed that record a transfer the four-step way — a sale and a withdrawal in one account, a deposit and a purchase in another, the sale and the purchase of one asset — with the one transfer they stand for: give their ids, as list_transactions gives them, and a rate between currencies if not the stored one. The transfer goes from the sale's account to the purchase's, on the sale's date, the sale's quantity sent and the purchase's received, at its cost. What the window's transfer would refuse on the accounts without the four is refused before asking. The owner is shown the four and the transfer, and answers them alone; the call waits for that answer. Answered replaced, with the transfer, or refused with ReplacementRefused, which gives nothing else. The transfer is the owner's: it is not marked as an agent's.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "transactions": { "type": "array", "items": { "type": "string" }, "description": "The ids of the four transactions, as list_transactions gives them." },
                    "rate": { "type": ["string", "number"], "description": "Between accounts of different currencies, the rate from the sending account's currency to the receiving one's; the stored rate of the sale's date when not given." }
                },
                "required": ["transactions"],
                "additionalProperties": false
            }),
        },
    ]
}

/// A deposit or a withdrawal as an agent is told of it.
fn cash_definition(
    name: &'static str,
    title: &'static str,
    description: &'static str,
) -> ToolDefinition {
    ToolDefinition {
        name,
        title,
        only_reads: false,
        description,
        input_schema: json!({
            "type": "object",
            "properties": {
                "account": { "type": "string", "description": "The account's name, as list_accounts gives it." },
                "amount": { "type": ["string", "number"], "description": "The amount, in the account's currency." },
                "date": { "type": "string", "description": "YYYY-MM-DD, today or earlier. Default: today." },
                "note": { "type": "string", "description": "A note kept with the transaction." }
            },
            "required": ["account", "amount"],
            "additionalProperties": false
        }),
    }
}

/// A purchase or a sale as an agent is told of it.
fn trade_definition(
    name: &'static str,
    title: &'static str,
    description: &'static str,
) -> ToolDefinition {
    ToolDefinition {
        name,
        title,
        only_reads: false,
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

/// AGT-052 — whether `tool` is one that only reads, as its definition says.
pub fn only_reads(tool: &str) -> bool {
    definitions()
        .iter()
        .any(|definition| definition.name == tool && definition.only_reads)
}
