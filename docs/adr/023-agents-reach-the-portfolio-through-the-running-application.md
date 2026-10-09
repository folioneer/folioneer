# ADR 023 — An agent reaches the portfolio only through the running application

**Date**: 2026-10-09
**Status**: Accepted

## Context

An agent client (Claude Code, Claude Desktop) calls an application's tools through the Model Context Protocol: it starts a program and talks to it over that program's standard input and output. The owner wants an agent to read the portfolio and record in it, on three conditions: nothing answers when the application is closed, the owner allows each connection in the window, and the window shows at once what an agent recorded.

The program an agent client starts is therefore not the open application: it is a second process of the same executable. What that second process may touch decides whether the three conditions can hold.

## Decision

**The program started with `--mcp` is a bridge that holds no data.** It opens no database and reads no file of the portfolio. It passes each request to the running application over a local channel and returns the answer; when no application answers, it says the application is not open.

**The channel is local to the owner's user and is never a network port.** Only a program of the owner's user can reach it, and the application checks that the program at the other end runs as that user. The channel exists only while the owner's setting allows agents to connect.

**The running application alone holds the grant and executes every request**, with the queries and commands the window uses. One process writes the portfolio, and the window learns of a recording the way it learns of its own.

Alternatives considered:

- **The bridge opens the database itself** — rejected. It would answer with the application closed, write beside the window (two writers, and a window that does not know what changed), and leave nowhere for the owner to allow or refuse a connection.
- **A port on the local network interface** — rejected. Any program of any user on the machine can reach it, and so can a page in a browser; a secret would then have to be stored and handed to the agent client, which is one more thing to protect.
- **The agent client talks to the application without a bridge** — rejected. Agent clients start a program and use its standard input and output; a socket file is not something they connect to.

## Consequences

- **Pros**: closing the application or switching the setting off ends every agent's access, with nothing left listening; consent lives in the window, where the owner is; the rules that guard a recording are the window's, with no second copy to keep in step.
- **Cons**: an agent cannot work while the application is closed — the command line remains the way to record then (CLI-030). Each system needs its own channel: a system without one has no agent connection until it is written.

## Guard

- **Reversal looks like**: a database pool, a repository, a service, a use case or a file of the data folder — the channel apart — in the bridge (`src-tauri/src/agent_connection/bridge.rs`) or in a file it uses; a `TcpListener`, a `UdpSocket` or an HTTP server anywhere in `src-tauri/src/agent_connection/`; a channel another user's program can open.
- **Guard**: in `src-tauri/src/agent_connection/`, `adr_023_the_bridge_is_made_of_files_that_reach_no_data` (the bridge uses only files that name no database, service or file — seen red once), `adr_023_the_agent_connection_opens_no_network_socket` (every file of the folder is scanned), `adr_023_the_bridge_answers_that_the_application_is_not_open`, and `agt_021_another_users_process_cannot_open_the_channel`; `reviewer-arch` and `reviewer-security` match the sign.
