# TODO-087 — (backend) — An agent reads figures in the unit it writes them

From gh244 (owner, 2026-10-12), which this entry closes when it ships. The agent connection speaks two units: a tool that records takes decimals (`12.5`), and everything an agent is answered is in millionths (`12500000`), percentages included. Each description says so; an agent that reads a quantity and writes it back, or repeats a percentage, is wrong by a million.

Read on 2026-10-13: it is wider than the issue says. The answer of a recording is in millionths too — `record_purchase` takes `12.5` and answers the transaction with `quantity: 12500000` — so the two units meet inside one call, and `list_transactions` and `get_price`, added in 0.10.0, answer in millionths as well. The command line prints the same millionths in its JSON (CLI-021), by its own rule: it is another interface and not part of this entry unless the owner says so.

**User value:** An agent cannot mis-scale a figure between reading and recording: fewer wrong recordings, fewer wrong answers to the owner.
**Done when:** every figure the agent connection answers and every figure it takes are in one unit, as decided below; `initialize` says the convention once (`instructions`), and the tools' descriptions no longer each explain it; a test reads a holding and records its quantity back unchanged; the agent's spec says so (AGT-040, AGT-044, AGT-070, AGT-071).
**Design:** none
**Open questions:**

- [x] Decimals everywhere — answers become decimal text (`"12.5"`, `"199.96"`) — or millionths kept with the decimal beside each figure? (Recommended: decimals everywhere; millionths stay inside the application.) — Decimals everywhere: every figure an agent reads is exact decimal text with up to six decimals, as it writes them; the application's six decimals are kept, nothing rounded. (owner, 2026-10-18)
- [x] Changing what a read answers breaks a client written against 0.10.0: is there one to keep, besides Claude reading the descriptions each time? — None to keep: the answers switch in one release, said in the changelog and the connection's instructions. (owner, 2026-10-18)
- [x] Does the command line's JSON follow, or keep its millionths (CLI-021)? — It follows: decimal text in the command line's JSON too, CLI-021 changed in the same release. (owner, 2026-10-18)
