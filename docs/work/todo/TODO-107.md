# TODO-107 — (fullstack) — An asset created twice is merged into the other

From TODO-086 (owner, 2026-10-18), on gh243. Two computers that each create the same asset before syncing end with both (AST-033); the core reports the pair (AST-035, `SameAsAnother`) and TODO-086 shows it, but nothing settles it: editing one copy leaves its transactions and prices on it, and the rules refuse to make it a different asset than it is. It is the one anomaly of TODO-086 that keeps arising in normal use, for anyone who syncs two computers.

**User value:** An owner who sees an asset twice keeps one, with every transaction and price of both, in one step.
**Done when:** an asset reported as the same as another offers "Merge into <the other>"; after a confirmation that names both and what moves, every transaction, price and note of the copy is the other's, holdings and figures are those of the two together, and the copy is gone; the merge is one change that sync carries, so every computer ends with one asset; the golden portfolio's figures do not move; TODO-086's button for a duplicate opens it.
**Design:** none
**Open questions:**

- [x] Which of the two is kept: the older, the one with more transactions, or the owner's choice in the confirmation? — The owner's choice: the confirmation shows both side by side (name, reference, transactions), the older preselected. (owner, 2026-10-18)
- [x] Two prices on the same date, one on each copy: the kept asset's, the newer, or the owner's choice? — The kept asset's own price for that date; the copy's is dropped, the confirmation saying how many. (owner, 2026-10-18)
- [x] Can a merge be undone, or is it final once confirmed? — Final once confirmed, like deleting an asset; a backup (TODO-096) restores it. (owner, 2026-10-18)
