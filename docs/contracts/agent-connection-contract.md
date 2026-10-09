# Contract — Agent Connection

> Domain: `agent-connection`
> Last updated by: agent-connection (AGT-022–036)

> **Error model on the wire**: each command's error serializes as a flat `{ code: "VariantName" }` object. The FE matches on `code`.
>
> This contract covers the commands between the window and the core. The bridge, the channel and the tools (AGT-010–021, AGT-037–042) are not Tauri commands: what an agent, the bridge and the application exchange — the tools' refusal codes included — is described by the spec.

---

## Commands

| Command                      | Args                             | Return                 | Errors                                   |
| ---------------------------- | -------------------------------- | ---------------------- | ---------------------------------------- |
| `get_agent_connection_state` | —                                | `AgentConnectionState` | _(infallible — reads what is in memory)_ |
| `set_agents_allowed`         | `allowed: bool`                  | `AgentConnectionState` | `NoAgentChannel`, `AgentChannelFailed`   |
| `answer_agent_connection`    | `request_id: u32`, `allow: bool` | `()`                   | `ConnectionRequestGone`                  |
| `disconnect_agent`           | `session_id: u32`                | `()`                   | `SessionAlreadyEnded`                    |

- `get_agent_connection_state` — AGT-036. Called by the settings page (the setting) and by the shell (the dialog and the header).
- `set_agents_allowed` — AGT-022. `NoAgentChannel`: this system has no agent connection (AGT-023). `AgentChannelFailed`: the setting or the channel could not be written — another running application holds the channel, its folder is not a folder of the data folder — and the cause is in the log. A switch on that fails leaves the setting off. A switch off that fails has still closed the channel and ended every session: the window reads the state again.
- `answer_agent_connection` — AGT-032. `ConnectionRequestGone`: the request was already answered, or its agent client went away (AGT-030).
- `disconnect_agent` — AGT-034. `SessionAlreadyEnded`: the agent already left or was disconnected.

## Shared Types

```rust
// AGT-036 — what the window shows of the agent connection.
struct AgentConnectionState {
    available: bool,                       // this system has an agent connection (AGT-023)
    allowed: bool,                         // the owner's setting (AGT-022)
    user: Option<String>,                  // the user the application runs as; None when the system does not say
    requests: Vec<AgentConnectionRequest>, // waiting for the owner's answer, oldest first
    sessions: Vec<AgentSession>,           // connected, oldest first
}

// AGT-030 — an agent client waiting for the owner's answer.
struct AgentConnectionRequest {
    id: u32,
    client: String,   // the name it gave, cleaned and at most 60 characters (AGT-037)
    asked_at: String, // RFC 3339, this computer's time
}

// AGT-033 — a connected agent client.
struct AgentSession {
    id: u32,
    client: String,
    calls: u32,       // the tool calls it made
}

// Serialized as `{ code: "VariantName" }`.
enum AgentConnectionError {
    NoAgentChannel,
    AgentChannelFailed,
    ConnectionRequestGone,
    SessionAlreadyEnded,
}
```

## Events

| Event                    | Payload | Published when                                                                                                           |
| ------------------------ | ------- | ------------------------------------------------------------------------------------------------------------------------ |
| `AgentConnectionChanged` | —       | the setting is switched; a request arrives, is answered or is withdrawn; a session opens, calls a tool or ends (AGT-036) |

## Changelog

- 2026-10-09 — Created: the four commands, their types and the event (AGT-022–036).
