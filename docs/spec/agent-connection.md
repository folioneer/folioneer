# Business Rules — Agent Connection (AGT)

## Context

An agent — Claude Code, Claude Desktop, any program that speaks the Model Context Protocol — may read the portfolio and record in it through the application its owner has opened. The agent starts the installed program with `--mcp`; that process is a **bridge**: it holds no data and passes each tool call to the running application (ADR-023). The **channel** is what the application listens on; a **connection** is one bridge's link to it. The running application asks its owner before serving a connection and runs every tool through the queries and the recording rules the window uses. Nothing answers when the application is closed or when the owner's setting is off. The text an agent reads is English, like the command line's.

The agent connection exists on Linux and other Unix systems, and on Windows. A system without a local channel of either kind has none (AGT-023).

## Entity Definition

One stored entity, the mark of a transaction an agent recorded (AGT-045), in its own table beside the transactions. The rest is held in memory by the application, until it closes:

| Entity             | Field                | Type    | Meaning                                                                    |
| ------------------ | -------------------- | ------- | -------------------------------------------------------------------------- |
| Connection request | `id`                 | integer | identifies the request in the owner's answer                               |
|                    | `client`             | text    | the name the agent gave, cleaned (AGT-037)                                 |
|                    | `asked_at`           | instant | when it asked, in this computer's time                                     |
| Agent session      | `id`                 | integer | identifies the session when the owner disconnects it                       |
|                    | `client`             | text    | the name the agent gave, cleaned (AGT-037)                                 |
|                    | `calls`              | integer | the tool calls it made                                                     |
|                    | `recorded`           | integer | the transactions it recorded, counted against its limit (AGT-047)          |
|                    | `key`                | text    | identifies the session in what it records, never used again by another run |
|                    | `started_at`         | instant | when the owner allowed it                                                  |
| Mark (AGT-045)     | `transaction`        | text    | the transaction it marks: one mark at most per transaction                 |
|                    | `agent`              | text    | the name of the agent that recorded the transaction                        |
|                    | `session`            | text    | the `key` of the session that recorded it                                  |
|                    | `session_started_at` | instant | the `started_at` of that session                                           |
| State (AGT-036)    | `available`          | boolean | this system has an agent connection (AGT-023)                              |
|                    | `allowed`            | boolean | the owner's setting (AGT-022)                                              |
|                    | `user`               | text    | the user the application runs as, when the system says it                  |
|                    | `requests`           | list    | the connection requests waiting, oldest first                              |
|                    | `sessions`           | list    | the agent sessions open, oldest first                                      |

Two things are kept on the computer across restarts, and neither is synced to the owner's other computers: the owner's setting "Allow agents to connect", as a file `agents-allowed` in the application's data folder; and the marks (AGT-045).

---

## Business Rules

### The bridge (010–019)

**AGT-010 — The bridge speaks the Model Context Protocol (backend)**: The program started with `--mcp` reads JSON-RPC messages, one per line, on its standard input and answers on its standard output until the input ends. It answers `initialize` (with the protocol version the agent asked for, the capability `tools`, and its name and version), `ping`, `tools/list` (the tools of AGT-046, each with a description and the JSON schema of its arguments) and `tools/call`; a notification takes no answer; any other method, and a call of a tool it does not list, is a JSON-RPC error. A line that is not JSON is skipped.

**AGT-011 — No application, no answer (backend)**: The bridge opens no database and reads no file of the portfolio (ADR-023). When no application answers on the channel — it is closed, or its setting is off — the bridge still introduces itself and lists its tools, and a tool call is answered with a refusal saying that Folioneer is not open or that agents are not allowed to connect.

**AGT-012 — `--mcp` starts the bridge (backend)**: `--mcp` as the program's first argument starts the bridge and nothing else: no window, no command of the command line (CLI-010), no scheduled download. Anywhere else among the arguments it does not start the bridge: the first argument decides, as CLI-010 says.

### The channel (020–029)

**AGT-020 — One request, one answer, one line (backend)**: The bridge and the application exchange JSON objects, one per line: the bridge asks to connect or calls a tool; the application answers that the connection is granted, with a tool's result, or with a refusal carrying a code and a reason. The code stays on the channel: the bridge passes the agent the reason, which is a full sentence, so a rule that names a refusal's code names what the application answers, not what the agent reads. A line that is no such object, or that is longer than 4 MiB — a tool's result included — ends the connection.

**AGT-021 — The channel is the owner's user's alone (backend)**: On Linux and other Unix systems the channel is a socket in a folder of the application's data folder. The folder is created for the owner's user alone (no permission for the group or for others), whatever the data folder allows, so no other user's program can reach the socket — the one thing of the data folder the bridge opens. On Windows the channel is a named pipe: its access list names the owner's user and nobody else, so the system refuses any other user's program that tries to open it, and it refuses clients of another computer; its name is drawn at each opening and written to a file of the same folder of the data folder, the one thing of the data folder the bridge opens. There the folder is not closed further: it takes the access list of the data folder, which is the owner's alone where the system puts it. The bridge opens only a name made of `\\.\pipe\folioneer-agent-` followed by letters, digits and dashes, and reads at most 256 bytes of the file. On both systems the application checks that the program at the other end runs as the same user and turns away one that does not, which is written to the log; on Windows the bridge checks the same of the program that answers, so a pipe another user opened under a name left by an earlier run is not spoken to. A file that names no pipe of the channel, or a pipe another user's program answers on, is to the agent an application that is not open (AGT-011). The channel is never a network port.

**AGT-022 — The channel exists only while the owner allows agents (frontend + backend)**: The setting "Allow agents to connect" is off until the owner switches it on, and is kept on this computer across restarts. With it off no channel exists. Switching it on opens the channel; the application opens it at start when the setting is on. Switching it off removes the channel and ends every grant (AGT-035). The settings page shows the setting with what it means.

**AGT-023 — A system without a channel has no agent connection (frontend + backend)**: Linux, the other Unix systems and Windows have a channel (AGT-021); the rule holds for a system the channel is not written for. Where the application has no local channel, the setting cannot be switched on (`NoAgentChannel`), the settings page shows it disabled and says the agent connection is not available on this system yet, and the bridge answers as AGT-011 says.

**AGT-024 — A channel that cannot be opened leaves the setting off (backend)**: A socket left by an earlier run — on Windows, the file naming its pipe — is replaced; so is, on Windows, a name whose pipe another user's program answers on. The channel is not opened in four cases: its folder is a symbolic link (on Windows, a junction too), its folder cannot be created, its socket — or its pipe, or the file naming it — cannot be created, or a running application of the owner's user still answers on its socket or its pipe — that application keeps its channel. On Windows two applications opening at the same moment are told apart by a lock on the file naming the pipe: the second finds the first one's pipe. Switching the setting on is then refused with `AgentChannelFailed`, the setting stays off, and the cause is written to the log. At start, with the setting kept on, the same cases leave the setting as it is and no channel: the cause is written to the log, and nothing listens until the application starts again or the owner switches the setting off and on. An application that opened no channel removes none when it closes.

### Consent (030–039)

**AGT-030 — A connection waits for the owner (backend)**: The bridge asks to connect at an agent's first tool call, under the name the agent gave for itself. The application answers only when its owner has: nothing is granted by default or after a delay, and the agent's call waits as long as the owner's answer does. An agent that goes away before the answer withdraws its request.

**AGT-031 — Nothing runs before the owner allowed (backend)**: A tool call on a connection the owner has not allowed is refused and reaches no tool. A refusal ends that request; the bridge then closes its connection, so the agent's next call asks again.

**AGT-032 — The owner allows or refuses in the window (frontend + backend)**: While an agent waits, a dialog shows over whatever view is open: the agent's name, the user it runs as — the application's own, the only one whose programs can connect (AGT-021) — the time it asked, what a yes covers — reading the accounts, the assets, the holdings and the totals, and recording opening balances, purchases and sales, up to the session's limit (AGT-044, AGT-047) — that one answer covers the whole session, and what it never covers: changing, removing or undoing a transaction, even one the agent recorded, and reading a file, a setting or the sync folder. Its two actions are "Allow for this session" and "Refuse". It is easy to refuse and hard to approve blindly: refusing is the filled action and has the focus; "Allow" is inactive for a moment after the dialog appears; the keyboard's focus stays on the two actions; and the dialog closes only by an answer — neither its corner nor its backdrop dismisses it. A refusal gives the agent nothing: its call is answered that the connection was refused. Several agents waiting are shown one at a time, oldest first.

**AGT-033 — One grant covers the session (frontend + backend)**: Once allowed, the agent's calls on that connection are served without another question, and each is counted. While an agent is connected the header says so — "{agent} is connected" — on every view; a long name is shown cut, with the whole name as its hint. Nothing of a grant is remembered once the session ends, except on what it recorded (AGT-045).

**AGT-034 — The owner disconnects, or the agent leaves (frontend + backend)**: The header offers "Disconnect" beside each connected agent. Disconnecting closes that agent's connection, before anything it already sent is read: its next call is answered that Folioneer closed the connection, and the one after asks to connect again. The channel and the other agents' connections are untouched. An agent that goes away ends its own session.

**AGT-035 — Closing the application or the setting ends every grant (backend)**: When the setting is switched off, every request is refused, every session ends, every connection is closed — one that had not asked yet included — and the channel is removed, the file naming the pipe with it. When the application closes, the requests, the sessions and the connections end with it; the socket file, or the file naming the pipe, may stay, answers nothing — a bridge that tries it is told the application is not open (AGT-011) — and is replaced at the next start (AGT-024). No request is shown and no session opens afterwards, even for an answer given at that moment.

**AGT-036 — The window shows what the core holds (frontend + backend)**: The window reads the state of the agent connection (see Entity Definition). Every change — the setting, a request, a session, a tool call — publishes `AgentConnectionChanged`, on which the window reads the state again.

**AGT-037 — An agent's name is text the application did not write (backend)**: The name an agent gives is cleaned before it is shown or logged, by the bridge and again by the application, since a program may speak to the channel without the bridge: control characters and the invisible characters that reorder or hide text are removed, it is cut to 60 characters, and an empty one becomes "An agent".

**AGT-038 — Limits (backend)**: At most 8 bridges are connected at once and at most 4 agents wait for the owner's answer; one more is turned away, and its call is answered as a refusal. A connection that has not asked to connect within 30 seconds is closed.

**AGT-039 — An answer that comes too late (frontend + backend)**: Answering a request that was withdrawn is refused with `ConnectionRequestGone`; disconnecting a session that already ended, with `SessionAlreadyEnded`. The window says so and reads the state again.

### Tools (040–049)

**AGT-040 — The read tools (backend)**: A connected agent may call four tools, each a query the window makes, answered as JSON text: `portfolio_summary` — each account with its total value, its unrealized gain and its performance this year, and the portfolio total (ACC-021, ACC-027); `list_accounts` — the accounts by name with their currency (CLI-018); `list_assets` — the assets a holding can be of, never a Cash Asset, with the fields of the command line's asset rows (CLI-021), the archived ones too when `archived` is true; `list_holdings` — the holdings of the account named by `account` with their figures and the account's totals (ACD), as they stood on the date `as_of`, today or earlier, when it is given. Amounts and quantities are the core's micro-units, which each description says. The schemas declare no other argument; one that arrives all the same is ignored.

**AGT-041 — Every call is written to the log (backend)**: Each tool call is written to the application's log with its session, its tool and whether it was refused; a connection allowed or refused is written with the agent's cleaned name (AGT-037).

**AGT-042 — A refusal names only what the agent sent (backend)**: The reason of a refusal repeats what the agent sent and never lists the accounts or the assets that exist — the list tools are how to ask.

**AGT-043 — The refusals of the read tools (backend)**: `list_holdings` names its account by its name, case ignored: a name that matches no account is refused with `AccountNotFound`, one that two accounts share with `AccountAmbiguous`, a missing one with `MissingArgument`; an `as_of` that is not a date, or is in the future, with `InvalidDate`. A read that fails is refused with `DatabaseError`, in any of the four tools.

**AGT-044 — The three recordings (frontend + backend)**: A connected agent may record what the command line records, through the same recorder and the same rules as the window (CLI-012): `record_opening_balance` — the quantity and the total cost of what an account already held (TRX-047); `record_purchase` and `record_sale` — a quantity with either the unit price in the asset's currency or the broker's all-in total in the account's currency (CLI-013), and optionally fees, an exchange rate and a note of at most 500 characters. Each names its account by name and its asset by name, reference, ISIN or reference with its exchange (CLI-011). What is left out is as on the command line (CLI-014): the date is today, the fees 0, the rate 1. A recording is answered with the transaction as recorded, its account, its currency and its asset. The window shows it as it happens, by the event every recording publishes.

**AGT-045 — What an agent recorded is marked (frontend + backend)**: A transaction an agent recorded is marked with the agent's name, the key of its session and the time that session started; a transaction without a mark is one the owner typed, one an agent recorded on another of the owner's computers — the mark is kept on the computer that recorded it and is not sent to the others — or, should both the marking and its undoing fail, the one AGT-049 names. The rows of the account journal carry the mark (TXL-060), and the journal shows it in a column "Recorded by": the agent's name, with "{agent} · session of {date and time}" as its hint (TXL-061). The mark goes with its transaction: correcting the transaction keeps it, removing the transaction removes it.

**AGT-046 — No other tool exists (backend)**: The tools are the four of AGT-040 and the three of AGT-044. Nothing corrects or deletes a transaction, creates an account or an asset, or takes or returns a path, a setting or anything of sync.

**AGT-047 — A session records up to a limit (backend)**: One session records at most 500 transactions. The next recording is refused with `SessionLimitReached`, which says the owner disconnects and allows a new session to go on; reading is not limited.

**AGT-048 — The refusals of a recording (backend)**: Figures are decimals with a dot and at most six decimals, as text or as JSON numbers; anything else is refused with `InvalidFigure`; a missing figure, account or asset with `MissingArgument`. An argument a recording does not know is ignored, as for the read tools (AGT-040). A price together with a total, or neither, a `date` or a `note` that is not text, and a note that is too long are refused with `InvalidArguments` — a date that is not text is never read as today. Every other refusal is the recording's own, with its code and its reason: the account or the asset not found or ambiguous (CLI-011), a date that is text but not a date (`InvalidDate`) or is in the future (`DateInFuture`), an oversell, and the rest of the window's rules (CLI-012).

**AGT-049 — A recording that cannot be marked is undone (backend)**: Recording and marking are one operation of the core, which runs to its end once started: a disconnect or a closing application never stops it between its two writes. When the mark cannot be written the recording is undone and the agent is refused with `NotRecorded`: nothing was recorded. Should the undoing fail too, the refusal is `RecordedNotMarked`, which says the transaction was recorded and tells the agent to tell the owner; both failures are written to the log, and that transaction counts against the session's limit (AGT-047).

---

## Workflow

```
[Agent] ──starts──▶ folioneer --mcp (bridge: no data)
      │ initialize, tools/list                 answered by the bridge alone (AGT-010)
      │ tools/call ──▶ bridge ──connect──▶ channel ──▶ running application
      │                                                  │ setting off / closed → "not open" (AGT-011)
      │                                                  ▼
      │                                   dialog: Allow for this session / Refuse (AGT-032)
      │                                                  │ refused → nothing (AGT-031)
      │                                                  ▼ allowed
      │ ◀── result ── bridge ◀── answer ── tool run through the window's queries and rules (AGT-040, AGT-044)
      ▼
header: "{agent} is connected" · Disconnect (AGT-033, AGT-034)
```

---

## UX Mockup

### Entry point

**Settings** — "Allow agents to connect", under the sync line. The connection dialog and the header's indicator appear on their own.

### States

- **Setting off**: nothing listens; the checkbox says what switching it on allows.
- **Not available on this system**: the checkbox is disabled and says so (AGT-023).
- **An agent asks**: the connection dialog, refusing in focus (AGT-032).
- **Connected**: the header names the agent and offers to disconnect (AGT-033).
- **Too late**: a request already withdrawn or a session already ended is said in a snackbar, and the state is read again (AGT-039).

---

## Cross-amendments

- **CLI-030** — the command line records only while the window is closed; an agent works only while it is open. The two never write at once.
- **CLI-010** — `--mcp` is one of the program's starts without a window (AGT-012).
- **CLI-011, CLI-012, CLI-013, CLI-014** — an agent's recordings name their account and asset, take the same defaults, are checked and are refused as the command line's are (AGT-044, AGT-048).
- **TXL-060, TXL-061** — the rows of the account journal carry the mark, and the journal shows which agent session recorded a transaction (AGT-045).
- **SYN-023** — the marks are device-local data: they are never synced (AGT-045).
- **ADR-023** — the bridge holds no data and the channel is never a network port.

## Open Questions

None.
