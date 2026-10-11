# TODO-068 — (fullstack) — A lock on a price, so that nothing replaces it (deferred)

Raised by the owner on 2026-10-11, with TODO-059. Every price follows latest-write-wins (MKT-025): a fetch, a recording by an agent or the owner's own typing replaces the price an asset has on a date, and the source says what the price is. Where one price must stand — a figure the owner checked against a statement — the owner locks it, and it keeps its value until unlocked. It is a specification, not an ADR (owner, 2026-10-11): a rule of the market-price spec, per price (an asset on a date), beside the per-asset refresh lock that keeps an asset out of the fetch (MKT-150+).

Not urgent: a replaced price is not dramatic, and its source is shown.

**User value:** A price the owner has checked stays as it is, whatever fetches or records after it.
**Done when:** the owner locks and unlocks a price in the price history; a locked price is replaced by no write the rules name, each refused or skipped with its reason; the lock shows on its row.
**Design:** needed
**Open questions:**

- [ ] What does the lock hold against: the fetch, the price history backfill, an agent, the command line, a change arriving from another computer — and the owner's own edit of that row, or does editing a locked price ask to unlock it first?
- [ ] Is the lock part of what sync carries, so that a price locked on one computer is locked on the others?
- [ ] Is a write on a locked price refused with its reason (an agent, the command line) or skipped in silence (the fetch, the backfill)?
