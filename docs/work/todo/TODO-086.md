# TODO-086 — (fullstack) — The assets to settle show where the owner works

From gh243 (owner, 2026-10-12), which this entry closes when it ships; it takes DEBT-085, deleted with it. Assets created before the ISIN existed have none; when kinds arrived, every asset without an ISIN became custom (AST-034), as specified. In the owner's portfolio a dozen listed shares, each on an exchange, are custom assets without an ISIN. The core knows it: each breaks a rule of its kind — a custom asset on an exchange (AST-031) — and is reported by `get_assets_to_settle` (AST-035). No screen and no command shows that report (DEBT-085, read again on 2026-10-13: the command has no caller in `src/features/assets/gateway.ts`); the owner found out through an agent.

The remedy exists: editing the asset to the listed kind with its ISIN keeps its identity, its transactions and its prices (AST-005, AST-031).

**User value:** The owner sees which assets the arrival of kinds left incomplete, and settles each in a few clicks, so that every listed instrument is known by its ISIN.
**Done when:** the assets to settle show where the owner decided below, each with its problem in plain words and a way to its edit dialog; for a custom asset on an exchange the dialog opens on the listed kind, the ISIN to type; a test holds that an asset settled that way keeps its transactions and its prices; DEBT-085 is deleted.
**Design:** none
**Open questions:**

- [x] Where do they show: a mark on their row of the assets table, a notice above the table, or both? (A screen changes: a design to validate. DEBT-085 asked the same.) — Only when some exist: a line above the assets table ("N assets need your attention — Show them", a filter) and a mark on each row, each anomaly in a plain sentence with a button to its fix. The anomalies and their fixes: a listed asset without an ISIN — add it; a custom asset with an ISIN — set it as listed, or remove the ISIN; a custom asset on an exchange — set it as listed, or remove the exchange; a class that does not fit its kind — choose the class again (each opens the edit form on that field); the same asset twice, created on two computers — merged into the other, a new entry of its own (TODO-107), the button waiting on it. (owner, 2026-10-18)
- [x] Does an agent see the report — a field of `list_assets`, or a tool of its own — so that it tells the owner? — Yes, in `list_assets`: each asset carries its anomaly, if any, in the same plain words; fixing one stays the owner's. (owner, 2026-10-18)
