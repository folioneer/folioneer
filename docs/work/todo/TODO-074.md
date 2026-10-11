# TODO-074 — (backend) — An agent lists an account's transactions, with their ids and who recorded them

From gh205 (owner, 2026-10-11). Before recording an account's past from a statement, an agent needs to see what the account already holds as transactions, and which of them it may correct or cancel. Today it reads holdings and totals, never the transactions; the id it needs for `correct_recording` and `cancel_recording` exists only in the answer of its own recording.

A read tool, `list_transactions`: an account, and optionally an asset, a range of dates, a kind and who recorded — the owner, an agent, or this session. Each transaction with its id, date, kind, asset, quantity, unit price or total, fees, rate, note, and who recorded it, as "Recorded by" says in the journal.

**Size:** M — one pull request: a read tool and a rule of the agent's spec.
**User value:** An agent does not record a transaction twice, and finds the id to correct or cancel without asking the owner.
**Done when:** the tool lists an account's transactions with those filters, oldest first, in pages; each says who recorded it, read from the mark (AGT-045); it counts as a read, never as a recording, and says it only reads once tools say so (TODO-065); the program started with `--mcp` lists it.
**Design:** none
**Open questions:**

- [x] The connection dialog says what a yes lets an agent read — "read your accounts, assets, holdings and totals;": what does it say once an agent lists transactions? — It names them: "read your accounts, assets, holdings, transactions, prices and totals;" ("lire vos comptes, vos actifs, vos positions, vos transactions, vos prix et vos totaux ;"). This entry adds "transactions" ("vos transactions") to the line, to AGT-032 and to the README; TODO-075 adds "prices" (owner, 2026-10-11).
