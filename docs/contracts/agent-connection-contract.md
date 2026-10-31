# Contract — AI Agent Connection

> Domain: `agent-connection`
> Last updated by: agent-connection (AGT-022–036, AGT-039, AGT-045, AGT-052–054, AGT-064, AGT-081, AGT-082, AGT-084, AGT-088, AGT-091)

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName" }` object; a variant with fields carries them beside `code`, as `TransactionNotRemoved { reason }` does. The FE matches on `code`.
>
> This contract covers the commands between the window and the core. The bridge, the channel and the tools (AGT-010–021, AGT-037, AGT-038, AGT-040–044, AGT-046–051, AGT-060–063, AGT-065A, AGT-070, AGT-071, AGT-075, AGT-080, AGT-087, AGT-090) and the tools of AGT-064 and AGT-068 are not Tauri commands — what the window shows of an added asset is `AgentSession.assets_added` and `last_asset` here, of a created account `accounts_added` and `last_account`; what an AI agent, the bridge and the application exchange — the tools' refusal codes included — is described by the spec.

---

## Commands

| Command                      | Args                               | Return                 | Errors                                                                                                        |
| ---------------------------- | ---------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------- |
| `get_agent_connection_state` | —                                  | `AgentConnectionState` | _(infallible — reads what is in memory)_                                                                      |
| `set_agents_allowed`         | `allowed: bool`                    | `AgentConnectionState` | `NoAgentChannel`, `AgentChannelFailed`                                                                        |
| `answer_agent_connection`    | `request_id: u32`, `allow: bool`   | `()`                   | `ConnectionRequestGone`                                                                                       |
| `answer_removal_request`     | `request_id: u32`, `remove: bool`  | `()`                   | `RemovalRequestGone` (AGT-081), `TransactionNotRemoved { reason }` (AGT-081), `DatabaseError` (AGT-081)       |
| `answer_change_request`      | `request_id: u32`, `change: bool`  | `()`                   | `ChangeRequestGone` (AGT-088), `TransactionNotChanged { reason }` (AGT-088), `DatabaseError` (AGT-088)        |
| `answer_replacement_request` | `request_id: u32`, `replace: bool` | `()`                   | `ReplacementRequestGone` (AGT-091), `TransactionsNotReplaced { reason }` (AGT-091), `DatabaseError` (AGT-091) |
| `disconnect_agent`           | `session_id: u32`                  | `()`                   | `SessionAlreadyEnded`                                                                                         |
| `remove_agent_recordings`    | `session_id: u32`                  | `SessionRemoval`       | `SessionAlreadyEnded`, `DatabaseError`                                                                        |

- `get_agent_connection_state` — AGT-036. Called by the Settings dialog (the setting) and by the shell (the dialog and the header).
- `set_agents_allowed` — AGT-022. `NoAgentChannel`: this system has no AI agent connection (AGT-023). `AgentChannelFailed`: the setting or the channel could not be written — another running application holds the channel, its folder is not a folder of the data folder — and the cause is in the log. A switch on that fails leaves the setting off. A switch off that fails has still closed the channel and ended every session: the window reads the state again.
- `answer_agent_connection` — AGT-032. `ConnectionRequestGone`: the request was already answered, or its AI agent client went away (AGT-030).
- `answer_removal_request` — AGT-081. Removes the transaction through the deletion of the window (TRX-034) when `remove`, or refuses; either way the AI agent that asked is told (AGT-080) and the request is gone. `RemovalRequestGone`: the request was already answered, or its session ended (AGT-082). `TransactionNotRemoved`: a yes removed nothing, and the transaction stays — `reason` is the code the AI agent is given: `TransactionChanged` (it no longer stands as the owner was shown it), `TransactionNotFound` (deleted meanwhile), or the deletion's own code (a later transaction depends on it). `DatabaseError`: the portfolio could not be read or written; the request is gone all the same.
- `answer_change_request` — AGT-088. Changes the transaction through the correction of the window (TRX-031), with what the AI agent asked, when `change`, or refuses; either way the AI agent that asked is told (AGT-087) and the request is gone. `ChangeRequestGone`: the request was already answered, or its session ended (AGT-082). `TransactionNotChanged`: a yes changed nothing, and the transaction stays as it was — `reason` is the code the AI agent is given: `TransactionChanged` (it no longer stands as the owner was shown it), `TransactionNotFound` (deleted meanwhile), or the correction's own code. `DatabaseError`: the portfolio could not be read or written; the request is gone all the same.
- `answer_replacement_request` — AGT-091. Removes the four transactions and records the transfer in one write when `replace`, or refuses; either way the AI agent that asked is told (AGT-090) and the request is gone. `ReplacementRequestGone`: the request was already answered, or its session ended (AGT-082). `TransactionsNotReplaced`: a yes replaced nothing, and the four stay as they were — `reason` is the code the AI agent is given: `TransactionChanged` (one no longer stands as the owner was shown it), `TransactionNotFound` (one deleted meanwhile), or the transfer's own code. `DatabaseError`: the portfolio could not be read or written; nothing was replaced, and the request is gone all the same.
- `disconnect_agent` — AGT-034. `SessionAlreadyEnded`: the AI agent already left or was disconnected.
- `remove_agent_recordings` — AGT-053. Removes every transaction the session recorded and that still exists, and nothing else. `SessionAlreadyEnded`: the session is no longer connected, and nothing was removed. `DatabaseError`: what it recorded could not be read, or a removal could not be saved; what was removed before the failure stays removed, and the state says what is left.

## Shared Types

```rust
// AGT-036 — what the window shows of the agent connection.
struct AgentConnectionState {
    available: bool,                       // this system has an agent connection (AGT-023)
    allowed: bool,                         // the owner's setting (AGT-022)
    user: Option<String>,                  // the user the application runs as; None when the system does not say
    requests: Vec<AgentConnectionRequest>, // waiting for the owner's answer, oldest first
    sessions: Vec<AgentSession>,           // connected, oldest first
    removals: Vec<RemovalRequest>,         // removal requests waiting for the owner's answer, oldest first (AGT-081)
    changes: Vec<ChangeRequest>,           // change requests waiting, oldest first; one order with `removals`, by `id` (AGT-088)
    replacements: Vec<ReplacementRequest>, // replacement requests waiting, oldest first; one order with `removals` and `changes`, by `id` (AGT-091)
}

// AGT-030 — an agent client waiting for the owner's answer.
struct AgentConnectionRequest {
    id: u32,
    client: String,   // the name it gave, cleaned and at most 60 characters (AGT-037)
    asked_at: String, // RFC 3339, this computer's time
}

// AGT-033 / AGT-052 — a connected agent client, with what its session did.
struct AgentSession {
    id: u32,
    client: String,
    calls: u32,                            // the tool calls it made
    started_at: String,                    // RFC 3339, this computer's time: when the owner allowed it
    reads: u32,                            // its calls of a read tool that were answered
    recordings: u32,                       // the transactions it recorded that still exist
    transactions_recorded: u32,            // the transactions it recorded, whatever became of them; only rises (AGT-084)
    last_recording: Option<LastRecording>, // the last recorded of those that still exist; None when none is left
    accounts_added: u32,                   // the accounts it created (AGT-068)
    last_account: Option<String>,          // the name of the last created of them; None when it created none
    assets_added: u32,                     // the assets it added (AGT-064)
    last_asset: Option<LastAsset>,         // the last added of them; None when it added none
}

// AGT-052 — the last transaction a session recorded.
struct LastRecording {
    kind: TransactionType, // account-contract.md
    asset: String,         // the asset's reference; its identifier when the asset cannot be read
    date: String,          // YYYY-MM-DD
}

// AGT-064 — the last asset a session added.
struct LastAsset {
    name: String,      // the asset's name
    reference: String, // the asset's reference
}

// AGT-080 / AGT-081 — an agent asking the owner to remove a transaction the owner typed.
struct RemovalRequest {
    id: u32,
    session: u32,                  // the `AgentSession.id` that asks
    client: String,                // the agent's name, cleaned (AGT-037)
    asked_at: String,              // RFC 3339, this computer's time
    transaction: TransactionShown, // the transaction as it stood when the agent asked
}

// AGT-087 / AGT-088 — an agent asking the owner to change a transaction the owner typed.
struct ChangeRequest {
    id: u32,                       // shares one rising order with `RemovalRequest.id`
    session: u32,                  // the `AgentSession.id` that asks
    client: String,                // the agent's name, cleaned (AGT-037)
    asked_at: String,              // RFC 3339, this computer's time
    transaction: TransactionShown, // the transaction as it stood when the agent asked
    becomes: TransactionShown,     // the transaction as the change would make it
    moved: Vec<ShownValue>,        // the values the change moves, in the order they are shown (AGT-088)
}

// AGT-090 / AGT-091 — an agent asking the owner to replace four transactions the owner typed with a transfer.
struct ReplacementRequest {
    id: u32,                             // shares one rising order with the removal and change requests' ids
    session: u32,                        // the `AgentSession.id` that asks
    client: String,                      // the agent's name, cleaned (AGT-037)
    asked_at: String,                    // RFC 3339, this computer's time
    transactions: Vec<ReplacedShown>, // the four as they stood when the agent asked: the sale, the withdrawal, the deposit, the purchase
    transfer: TransferShown,          // the transfer recorded in their place
}

// AGT-092 — one of the four a replacement removes, with what it moved, signed.
struct ReplacedShown {
    transaction: TransactionShown, // the transaction as it stood when the agent asked
    holding_moved: Option<i64>,    // micro-units: into the holding, negative out of it; None on the account's cash
    cash_moved: i64,               // micro-units: into the account's cash, negative out of it
}

// AGT-091 — the transfer a replacement records, as the owner is shown it.
struct TransferShown {
    from_account: String,         // the sending account's name
    to_account: String,           // the receiving account's name
    date: String,                 // YYYY-MM-DD: the sale's date
    asset_name: String,
    asset_reference: String,
    quantity_sent: i64,           // micro-units: the sale's quantity
    quantity_received: i64,       // micro-units: the purchase's quantity
    quantity_not_received: Option<i64>, // micro-units: quantity_sent − quantity_received, the fee in kind (TRF-021) the dialog names by its quantity alone (AGT-092); None when all is received
    cost: i64,                    // micro-units, in `from_currency`: the cost the transfer moves
    from_currency: String,        // the sending account's currency
    exchange_rate: Option<i64>,   // micro-units; None between accounts of one currency
    converted_cost: Option<i64>,  // micro-units, in `to_currency`; None between accounts of one currency
    to_currency: String,          // the receiving account's currency; `from_currency` within one currency
}

// AGT-088 — a value of a shown transaction a change can move; serialized snake_case.
enum ShownValue {
    Date,
    Quantity,
    Amount,
    UnitPrice,
    Fees,
    ExchangeRate,
    Note,
}

// AGT-081 / AGT-088 / AGT-091 — a transaction the owner typed, as the owner is shown it before removing, changing or replacing it.
struct TransactionShown {
    id: String,
    date: String,                    // YYYY-MM-DD
    account: String,                 // the account's name
    currency: String,                // the account's currency, which `amount` is in
    kind: TransactionType,           // account-contract.md
    asset_name: Option<String>,      // None for a transaction on the account's cash
    asset_reference: Option<String>, // None for a transaction on the account's cash
    quantity: i64,                   // micro-units
    amount: i64,                     // micro-units, in the account's currency
    unit_price: i64,                 // micro-units, in the asset's currency, as stored (AGT-088)
    fees: i64,                       // micro-units, in the account's currency, as stored (AGT-088)
    exchange_rate: i64,              // micro-units, as stored (AGT-088)
    note: Option<String>,            // as typed (AGT-088)
}

// AGT-053 — what removing everything a session recorded did.
struct SessionRemoval {
    removed: u32, // transactions removed
    kept: u32,    // recordings a later transaction depends on, left in place
}

// Serialized as `{ code: "VariantName" }`.
enum AgentConnectionError {
    NoAgentChannel,
    AgentChannelFailed,
    ConnectionRequestGone,
    SessionAlreadyEnded,
    RemovalRequestGone,                       // AGT-081
    TransactionNotRemoved { reason: String }, // AGT-081 — the deletion's code
    ChangeRequestGone,                        // AGT-088
    TransactionNotChanged { reason: String }, // AGT-088 — the correction's code
    ReplacementRequestGone,                   // AGT-091
    TransactionsNotReplaced { reason: String }, // AGT-091 — the code the agent is given
    DatabaseError,
}
```

## Events

| Event                    | Payload | Published when                                                                                                                                                                                                                                                                           |
| ------------------------ | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AgentConnectionChanged` | —       | the setting is switched; a request arrives, is answered or is withdrawn; a session opens, calls a tool or ends (AGT-036); the owner removes what a session recorded (AGT-053); a removal, change or replacement request arrives, is answered or is withdrawn (AGT-081, AGT-088, AGT-091) |

The mark of a transaction an AI agent recorded (AGT-045) reaches the window in the account journal: `JournalRow.recorded_by` in `account-contract.md`.

## Changelog

- 2026-10-09 — Created: the four commands, their types and the event (AGT-022–039).
- 2026-10-09 — AGT-045: the mark of an AI agent's recording reaches the window as `JournalRow.recorded_by` (`account-contract.md`); no command added.
- 2026-10-10 — TODO-060: `remove_agent_recordings` and `SessionRemoval` (AGT-053); `AgentSession` gains `started_at`, `reads`, `recordings` and `last_recording`, with `LastRecording` (AGT-052); `AgentConnectionError` gains `DatabaseError`.
- 2026-10-12 — TODO-063: `AgentSession` gains `assets_added` and `last_asset`, with `LastAsset { name: String, reference: String }` (AGT-064). No command or error changes.
- 2026-10-14 — TODO-061: `answer_removal_request` (AGT-081); `AgentConnectionState` gains `removals`, with `RemovalRequest` and `TransactionShown`; `AgentConnectionError` gains `RemovalRequestGone` and `TransactionNotRemoved { reason }`.
- 2026-10-14 — TODO-079: `AgentSession` gains `transactions_recorded` (AGT-084). No command or error changes.
- 2026-10-17 — TODO-080: `answer_change_request` (AGT-088); `AgentConnectionState` gains `changes`, with `ChangeRequest` and `ShownValue`; `AgentConnectionError` gains `ChangeRequestGone` and `TransactionNotChanged { reason }`; `TransactionShown` gains `unit_price`, `fees`, `exchange_rate` and `note`.
- 2026-10-24 — #386: `AgentSession` gains `accounts_added` and `last_account` (AGT-068). No command or error changes.
- 2026-10-24 — #512: the tool `record_transfer` (AGT-065A) is no Tauri command; its parts reach the window as `AgentSession.recordings` and `transactions_recorded` count them. No command, type or error changes.
- 2026-10-24 — #513: `answer_replacement_request` (AGT-091); `AgentConnectionState` gains `replacements`, with `ReplacementRequest` and `TransferShown`; `AgentConnectionError` gains `ReplacementRequestGone` and `TransactionsNotReplaced { reason }`. The tool `request_transfer_replacement` (AGT-090) is no Tauri command.
- 2026-10-24 — #513: `TransferShown` gains `quantity_not_received`, and `ReplacementRequest.transactions` are `ReplacedShown`, each with what it moved, signed (AGT-091, AGT-092). No command or error changes.
- 2026-10-25 — #339: the tool `list_prices` (AGT-075) is no Tauri command. No command, type or error changes.
