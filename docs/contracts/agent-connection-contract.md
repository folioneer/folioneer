# Contract — Agent Connection

> Domain: `agent-connection`
> Last updated by: agent-connection (AGT-022–036, AGT-039, AGT-045, AGT-052, AGT-053, AGT-064, AGT-080–082)

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName" }` object; a variant with fields carries them beside `code`, as `TransactionNotRemoved { reason }` does. The FE matches on `code`.
>
> This contract covers the commands between the window and the core. The bridge, the channel and the tools (AGT-010–021, AGT-037, AGT-038, AGT-040–044, AGT-046–051, AGT-060–064, AGT-070, AGT-071, AGT-080) are not Tauri commands: what an agent, the bridge and the application exchange — the tools' refusal codes included — is described by the spec.

---

## Commands

| Command                      | Args                              | Return                 | Errors                                                                                                  |
| ---------------------------- | --------------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------- |
| `get_agent_connection_state` | —                                 | `AgentConnectionState` | _(infallible — reads what is in memory)_                                                                |
| `set_agents_allowed`         | `allowed: bool`                   | `AgentConnectionState` | `NoAgentChannel`, `AgentChannelFailed`                                                                  |
| `answer_agent_connection`    | `request_id: u32`, `allow: bool`  | `()`                   | `ConnectionRequestGone`                                                                                 |
| `answer_removal_request`     | `request_id: u32`, `remove: bool` | `()`                   | `RemovalRequestGone` (AGT-081), `TransactionNotRemoved { reason }` (AGT-081), `DatabaseError` (AGT-081) |
| `disconnect_agent`           | `session_id: u32`                 | `()`                   | `SessionAlreadyEnded`                                                                                   |
| `remove_agent_recordings`    | `session_id: u32`                 | `SessionRemoval`       | `SessionAlreadyEnded`, `DatabaseError`                                                                  |

- `get_agent_connection_state` — AGT-036. Called by the settings page (the setting) and by the shell (the dialog and the header).
- `set_agents_allowed` — AGT-022. `NoAgentChannel`: this system has no agent connection (AGT-023). `AgentChannelFailed`: the setting or the channel could not be written — another running application holds the channel, its folder is not a folder of the data folder — and the cause is in the log. A switch on that fails leaves the setting off. A switch off that fails has still closed the channel and ended every session: the window reads the state again.
- `answer_agent_connection` — AGT-032. `ConnectionRequestGone`: the request was already answered, or its agent client went away (AGT-030).
- `answer_removal_request` — AGT-081. Removes the transaction through the deletion of the window (TRX-034) when `remove`, or refuses; either way the agent that asked is told (AGT-080) and the request is gone. `RemovalRequestGone`: the request was already answered, or its session ended (AGT-082). `TransactionNotRemoved`: a yes removed nothing, and the transaction stays — `reason` is the code the agent is given: `TransactionChanged` (it no longer stands as the owner was shown it), `TransactionNotFound` (deleted meanwhile), or the deletion's own code (a later transaction depends on it). `DatabaseError`: the portfolio could not be read or written; the request is gone all the same.
- `disconnect_agent` — AGT-034. `SessionAlreadyEnded`: the agent already left or was disconnected.
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
    last_recording: Option<LastRecording>, // the last recorded of them; None when none is left
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

// AGT-081 — a transaction the owner typed, as the owner is shown it before removing it.
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
    DatabaseError,
}
```

## Events

| Event                    | Payload | Published when                                                                                                                                                                                                                                  |
| ------------------------ | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `AgentConnectionChanged` | —       | the setting is switched; a request arrives, is answered or is withdrawn; a session opens, calls a tool or ends (AGT-036); the owner removes what a session recorded (AGT-053); a removal request arrives, is answered or is withdrawn (AGT-081) |

The mark of a transaction an agent recorded (AGT-045) reaches the window in the account journal: `JournalRow.recorded_by` in `account-contract.md`.

## Changelog

- 2026-10-09 — Created: the four commands, their types and the event (AGT-022–039).
- 2026-10-09 — AGT-045: the mark of an agent's recording reaches the window as `JournalRow.recorded_by` (`account-contract.md`); no command added.
- 2026-10-10 — TODO-060: `remove_agent_recordings` and `SessionRemoval` (AGT-053); `AgentSession` gains `started_at`, `reads`, `recordings` and `last_recording`, with `LastRecording` (AGT-052); `AgentConnectionError` gains `DatabaseError`.
- 2026-10-12 — TODO-063: `AgentSession` gains `assets_added` and `last_asset`, with `LastAsset { name: String, reference: String }` (AGT-064). No command or error changes.
- 2026-10-14 — TODO-061: `answer_removal_request` (AGT-081); `AgentConnectionState` gains `removals`, with `RemovalRequest` and `TransactionShown`; `AgentConnectionError` gains `RemovalRequestGone` and `TransactionNotRemoved { reason }`.
