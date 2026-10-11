# TODO-075 — (backend) — An agent reads an asset's price at a past date

From gh210 (owner, 2026-10-11). To check a rebuilt year end against a statement, an agent needs the price the application holds for an asset at that date. `list_holdings` with a past date gives a value, not the price, its date or its source.

A read tool, `get_price`: an asset and a date; the price at that date or the closest earlier one, with its own date, the asset's currency and its source — typed by the owner, fetched, or recorded by an agent (MKT-100).

**Size:** M — one pull request: a read tool and a rule of the agent's spec.
**User value:** An agent compares the application's valuation with a statement and sees a missing or a doubtful price before relying on it.
**Done when:** the tool answers the price at the date or the closest earlier one, with its date and its source; it says plainly when the asset has no price up to that date; it counts as a read and says it only reads once tools say so (TODO-065); the program started with `--mcp` lists it.
**Design:** none
**Open questions:**

- [x] The connection dialog says what a yes lets an agent read — "read your accounts, assets, holdings and totals;": what does it say once an agent reads prices? — It names them: "read your accounts, assets, holdings, transactions, prices and totals;" ("lire vos comptes, vos actifs, vos positions, vos transactions, vos prix et vos totaux ;"). This entry adds "prices" ("vos prix") to the line, to AGT-032 and to the README; TODO-074 adds "transactions" (owner, 2026-10-11).
