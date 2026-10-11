# TODO-071 — (backend) — An agent records a split and free shares

From gh209, refined with the owner on 2026-10-11: the urgent part. Rebuilding an account's past from statements meets stock splits, reverse splits and free shares; recorded as sales and purchases they show gains that never happened and break the cost basis. The application models both today — a split (SPL) and a free-share distribution (FSD) are transaction kinds with their rules, recorded in the window — and an agent can record neither.

Two more recordings for an agent, like the deposit and the dividend of TODO-059: a split — an account, an asset, a date and a ratio — and free shares — an account, an asset, a date and a quantity. The two are different operations, each with its own tool, its own rule and its own tests; they share this entry because the work is the same for both. The window's rules decide everything, fractions included; nothing new is modelled. The recordings go into the recorder the command line shares, so that its own commands (TODO-077) are only commands, help and printed lines; this entry adds none (owner, 2026-10-11: the window first, then the agent connection, then the command line).

**User value:** An agent rebuilds a history through a split or free shares with the right cost basis and no gain that never happened.
**Done when:** `record_split` and `record_free_shares` exist as tools, through the shared recorder and the window's rules (SPL, FSD), refused with their codes; each is marked with its session, counted against its limit, shown in the journal's "Recorded by", cancelled like any recording, and `correct_recording` says for each whether it corrects it or refuses it (AGT-061); the connection dialog names them in what a yes covers; the program started with `--mcp` lists them (`tests/agent_bridge_process.rs`).
**Design:** none
**Open questions:** none
